Ticket: Integrate Predictive Anomaly Detection for Alarm Management
Title: [Feature] Integrate AI Model API for Predictive Sensor Anomaly Detection in apps/core-server
Description:
We need to enhance our existing alarm management system by integrating an AI model (via external API) into the apps/core-server. Currently, alarms are triggered based on static thresholds. The goal of this task is to implement a predictive layer that analyzes sensor data streams to identify anomalies and forecast potential failures before hard thresholds are breached.
The core-server should act as the client, sending batched or streaming sensor data to the inference endpoint and processing the returned predictions to trigger or update system alarms.
Scope:
* API Integration: Implement a client service within apps/core-server to communicate with the AI model provider’s API.
* Data Pipeline: Hook into the existing sensor data ingestion flow to forward relevant metrics (e.g., temperature, humidity) to the AI model.
* Alarm Logic: Update the alarm evaluation logic to ingest anomaly scores/predictions. If the model predicts a critical anomaly, the system should trigger an alarm state, distinguishable from standard threshold alarms.
* Configuration: Add environment variables for API authentication (e.g., AI_MODEL_API_KEY, AI_MODEL_ENDPOINT).
Technical Context:
* The service is a FastAPI application (apps/core-server).
* Sensor data is currently orchestrated and stored (via TimescaleDB) within this service.
* The integration should be non-blocking to ensure standard data ingestion and existing alarm processing remains performant.
* Existing alarm types are likely defined in the database or code; this may require a new alarm type or an extension of the existing schema to support "predictive" or "anomaly" origins.
Acceptance Criteria:
* apps/core-server successfully authenticates and sends sensor data payloads to the configured AI API endpoint.
* The system parses the API response (anomaly score/prediction).
* A new alarm is generated or an existing alarm is escalated when the API response indicates a high probability of anomaly.
* Unit tests cover the API client interaction (mocked) and the updated alarm evaluation logic.
* Environment configuration is updated to support the new API integration.
Notes for Developer:
* Review apps/core-server for the optimal insertion point in the data processing pipeline (e.g., post-ingestion or via a background task).
* Ensure failure to reach the AI API does not crash the core application or block standard threshold-based alarms.
* Refer to the infra setup to ensure network egress is permitted for the native services.
