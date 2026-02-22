"""Listen for output commands published by the core server."""
from __future__ import annotations

import asyncio
import json
import logging
from datetime import datetime, timezone
from typing import Dict, Iterable, Optional, Set

from aiomqtt import Client, MqttError

from app.config import OutputConfig, Settings
from app.services.simulator import SimulatedDevice

logger = logging.getLogger(__name__)


class OutputCommandListener:
    """Subscribe to command topics and reflect state changes locally."""

    def __init__(self, settings: Settings, simulator: SimulatedDevice | None = None):
        self.settings = settings
        self.simulator = simulator
        self._task: asyncio.Task | None = None
        self._stop = asyncio.Event()
        self._output_topics: Dict[str, Set[str]] = {}

    def start(self) -> None:
        if self._task and not self._task.done():
            return
        self._stop.clear()
        self._build_topic_map()
        self._task = asyncio.create_task(self._run(), name="output-command-listener")

    async def stop(self) -> None:
        self._stop.set()
        if self._task:
            try:
                await self._task
            except asyncio.CancelledError:
                pass

    def _build_topic_map(self) -> None:
        """Map outputs to the topics they should listen to."""

        self._output_topics = {}
        default_pattern = f"iot/{self.settings.node_id}/{{output_id}}/command"
        for output in self.settings.outputs:
            topics = set()
            topics.add(default_pattern.format(output_id=output.output_id))
            if output.command_topic:
                topics.add(output.command_topic)
            self._output_topics[output.output_id] = topics

    async def _run(self) -> None:
        retry_delay = 2.0
        while not self._stop.is_set():
            try:
                async with Client(
                    self.settings.mqtt_host,
                    port=self.settings.mqtt_port,
                    username=self.settings.mqtt_username,
                    password=self.settings.mqtt_password,
                ) as client:
                    await self._listen(client)
            except asyncio.CancelledError:
                break
            except MqttError as exc:
                logger.warning("MQTT command listener error: %s", exc)
                await asyncio.sleep(retry_delay)
            except Exception:
                logger.exception("Unhandled error in command listener")
                await asyncio.sleep(retry_delay)

    async def _listen(self, client: Client) -> None:
        topics = self._all_topics()
        for topic in topics:
            await client.subscribe(topic)
        async for message in client.messages:
            if self._stop.is_set():
                break
            topic = getattr(message.topic, "value", None)
            if topic is None:
                topic = str(message.topic)
            await self._handle_message(client, topic, message.payload)

    def _all_topics(self) -> Iterable[str]:
        seen: Set[str] = set()
        seen.add(f"iot/{self.settings.node_id}/+/command")
        for entries in self._output_topics.values():
            seen.update(entries)
        return seen

    async def _handle_message(self, client: Client, topic: str, payload: bytes) -> None:
        output_id = self._output_for_topic(topic)
        if not output_id:
            logger.debug("Ignoring command for unknown topic %s", topic)
            return
        desired_state, reason, request_id = self._parse_command(payload)
        if not desired_state:
            logger.debug("Ignoring command without state for %s", topic)
            return

        result_state = desired_state
        stuck = False
        if self.simulator:
            result = self.simulator.apply_command(output_id, desired_state)
            result_state = result.applied_state
            stuck = result.stuck

        self._update_output_state(output_id, result_state)
        await self._publish_ack(client, output_id, desired_state, result_state, reason, request_id, stuck)

    def _output_for_topic(self, topic: str) -> Optional[str]:
        for output_id, topics in self._output_topics.items():
            if topic in topics:
                return output_id
        parts = topic.split("/")
        if len(parts) >= 4 and parts[0] == "iot" and parts[-1] == "command":
            return parts[-2]
        return None

    def _parse_command(self, payload: bytes) -> tuple[Optional[str], Optional[str], Optional[str]]:
        if not payload:
            return None, None, None
        text = payload.decode("utf-8", errors="ignore")
        reason = None
        request_id = None
        if text.startswith("{"):
            try:
                data = json.loads(text)
            except json.JSONDecodeError:
                return None, None, None
            state = data.get("state") or data.get("command")
            reason = data.get("reason")
            request_id = data.get("request_id")
            if state is None:
                return None, reason, request_id
            return str(state), reason, request_id
        return text.strip(), None, None

    def _update_output_state(self, output_id: str, state: str) -> None:
        for index, output in enumerate(self.settings.outputs):
            if output.output_id == output_id:
                updated = OutputConfig(
                    **output.model_dump(exclude={"default_state"}, exclude_none=True),
                    default_state=output.default_state,
                )
                updated.state = state
                self.settings.outputs[index] = updated
                break

    async def _publish_ack(
        self,
        client: Client,
        output_id: str,
        requested: str,
        applied: str,
        reason: Optional[str],
        request_id: Optional[str],
        stuck: bool,
    ) -> None:
        topic = f"iot/{self.settings.node_id}/{output_id}/state"
        payload = {
            "node_id": self.settings.node_id,
            "output_id": output_id,
            "requested": requested,
            "state": applied,
            "stuck": stuck,
            "reason": reason or "command",
            "ts": datetime.now(timezone.utc).isoformat(),
        }
        if request_id:
            payload["request_id"] = request_id
        try:
            await client.publish(topic, json.dumps(payload).encode("utf-8"))
        except Exception as exc:  # pragma: no cover - defensive log only
            logger.warning("Failed to publish ack for %s: %s", output_id, exc)
