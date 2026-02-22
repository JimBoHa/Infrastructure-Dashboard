/**
 * Strategy exports for Relationship Finder
 */

// Similarity strategy
export {
  JOB_TYPE as SIMILARITY_JOB_TYPE,
  buildSimilarityParams,
  generateSimilarityJobKey,
  validateSimilarityParams,
  SIMILARITY_PROGRESS_LABELS,
  getSimilarityProgressMessage,
} from "./similarity";
export type { SimilarityParams, SimilarityResult } from "./similarity";

// Correlation strategy
export {
  JOB_TYPE as CORRELATION_JOB_TYPE,
  buildCorrelationParams,
  generateCorrelationJobKey,
  buildCorrelationSensorIds,
  CORRELATION_PROGRESS_LABELS,
  getCorrelationProgressMessage,
} from "./correlation";
export type { CorrelationParams, CorrelationResult } from "./correlation";

// Events strategy
export {
  JOB_TYPE as EVENTS_JOB_TYPE,
  buildEventsParams,
  generateEventsJobKey,
  validateEventsParams,
  getEventsCandidateSensorIds,
  EVENTS_PROGRESS_LABELS,
  getEventsProgressMessage,
} from "./events";
export type { EventsParams, EventsResult } from "./events";

// Co-occurrence strategy
export {
  JOB_TYPE as COOCCURRENCE_JOB_TYPE,
  buildCooccurrenceParams,
  generateCooccurrenceJobKey,
  getCooccurrenceSensorIds,
  COOCCURRENCE_PROGRESS_LABELS,
  getCooccurrenceProgressMessage,
} from "./cooccurrence";
export type { CooccurrenceParams, CooccurrenceResult } from "./cooccurrence";
