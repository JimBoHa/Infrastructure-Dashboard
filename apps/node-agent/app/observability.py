from __future__ import annotations

import json
import logging
import sys
import uuid
from contextvars import ContextVar
from datetime import datetime, timezone
from typing import Any, Callable

from fastapi import FastAPI, Request, Response
from starlette.middleware.base import BaseHTTPMiddleware

try:  # pragma: no cover - optional runtime dependency
    from opentelemetry import trace
    from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
    from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
    from opentelemetry.sdk.resources import Resource
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor
    from opentelemetry.sdk.trace.sampling import TraceIdRatioBased
except Exception:  # pragma: no cover - allows running without otel extras
    trace = None
    OTLPSpanExporter = None
    FastAPIInstrumentor = None
    Resource = None
    TracerProvider = None
    BatchSpanProcessor = None
    TraceIdRatioBased = None


REQUEST_ID_HEADER = "X-Request-ID"
_request_id_ctx: ContextVar[str | None] = ContextVar("request_id", default=None)


def generate_request_id() -> str:
    return uuid.uuid4().hex


def get_request_id() -> str | None:
    return _request_id_ctx.get()


def set_request_id(value: str | None) -> None:
    _request_id_ctx.set(value)


class RequestIdMiddleware(BaseHTTPMiddleware):
    async def dispatch(self, request: Request, call_next: Callable[[Request], Any]) -> Response:
        inbound = request.headers.get(REQUEST_ID_HEADER) or request.headers.get("X-Correlation-ID")
        request_id = inbound or generate_request_id()
        token = _request_id_ctx.set(request_id)
        try:
            response = await call_next(request)
        finally:
            _request_id_ctx.reset(token)
        response.headers[REQUEST_ID_HEADER] = request_id
        return response


class TraceContextFilter(logging.Filter):
    def __init__(self, service: str) -> None:
        super().__init__()
        self._service = service

    def filter(self, record: logging.LogRecord) -> bool:
        record.service = self._service
        record.request_id = getattr(record, "request_id", None) or get_request_id()
        trace_id = None
        span_id = None
        if trace is not None:
            span = trace.get_current_span()
            if span:
                ctx = span.get_span_context()
                if ctx and ctx.trace_id:
                    trace_id = f"{ctx.trace_id:032x}"
                    span_id = f"{ctx.span_id:016x}"
        record.trace_id = trace_id
        record.span_id = span_id
        return True


class JsonLogFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        payload: dict[str, Any] = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "level": record.levelname,
            "message": record.getMessage(),
            "logger": record.name,
            "service": getattr(record, "service", None),
            "request_id": getattr(record, "request_id", None),
            "trace_id": getattr(record, "trace_id", None),
            "span_id": getattr(record, "span_id", None),
        }
        if record.exc_info:
            payload["exception"] = self.formatException(record.exc_info)
        extra = {
            key: value
            for key, value in record.__dict__.items()
            if key
            not in {
                "name",
                "msg",
                "args",
                "levelname",
                "levelno",
                "pathname",
                "filename",
                "module",
                "exc_info",
                "exc_text",
                "stack_info",
                "lineno",
                "funcName",
                "created",
                "msecs",
                "relativeCreated",
                "thread",
                "threadName",
                "processName",
                "process",
                "service",
                "request_id",
                "trace_id",
                "span_id",
            }
        }
        if extra:
            payload["extra"] = extra
        return json.dumps(payload, default=str)


def configure_logging(service: str, level: str = "INFO") -> None:
    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(JsonLogFormatter())
    handler.addFilter(TraceContextFilter(service))

    root = logging.getLogger()
    root.handlers = [handler]
    root.setLevel(level.upper())

    for logger_name in ("uvicorn", "uvicorn.error", "uvicorn.access"):
        logger = logging.getLogger(logger_name)
        logger.handlers = [handler]
        logger.setLevel(level.upper())
        logger.propagate = False


def _parse_header_pairs(raw: str | None) -> dict[str, str]:
    if not raw:
        return {}
    pairs: dict[str, str] = {}
    for item in raw.split(","):
        if "=" not in item:
            continue
        key, value = item.split("=", 1)
        key = key.strip()
        value = value.strip()
        if key:
            pairs[key] = value
    return pairs


def configure_tracing(
    *,
    service_name: str,
    service_version: str | None,
    otlp_endpoint: str,
    otlp_headers: str | None,
    sample_ratio: float = 1.0,
    app: FastAPI | None = None,
) -> None:
    if trace is None or OTLPSpanExporter is None:
        logging.getLogger(__name__).warning("OpenTelemetry not available; tracing disabled")
        return
    resource_attrs: dict[str, Any] = {"service.name": service_name}
    if service_version:
        resource_attrs["service.version"] = service_version
    sampler = None
    if TraceIdRatioBased is not None and sample_ratio is not None:
        try:
            ratio = float(sample_ratio)
        except (TypeError, ValueError):
            ratio = 1.0
        ratio = max(min(ratio, 1.0), 0.0)
        sampler = TraceIdRatioBased(ratio)
    provider = TracerProvider(resource=Resource.create(resource_attrs), sampler=sampler)
    span_exporter = OTLPSpanExporter(
        endpoint=otlp_endpoint,
        headers=_parse_header_pairs(otlp_headers),
    )
    provider.add_span_processor(BatchSpanProcessor(span_exporter))
    trace.set_tracer_provider(provider)
    if app is not None and FastAPIInstrumentor is not None:
        FastAPIInstrumentor.instrument_app(app)


def attach_request_id(payload: dict[str, Any], *, request_id: str | None = None) -> dict[str, Any]:
    if "request_id" in payload:
        return payload
    request_id = request_id or get_request_id()
    if request_id:
        payload["request_id"] = request_id
    return payload


def configure_observability(
    app: FastAPI,
    *,
    service_name: str,
    service_version: str | None,
    log_level: str,
    otel_enabled: bool,
    otlp_endpoint: str,
    otlp_headers: str | None,
    otel_sample_ratio: float = 1.0,
) -> None:
    configure_logging(service_name, log_level)
    app.add_middleware(RequestIdMiddleware)
    if otel_enabled:
        configure_tracing(
            service_name=service_name,
            service_version=service_version,
            otlp_endpoint=otlp_endpoint,
            otlp_headers=otlp_headers,
            sample_ratio=otel_sample_ratio,
            app=app,
        )
