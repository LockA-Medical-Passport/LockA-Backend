-- Reverses 20260916120000_init_schema.up.sql. Tables drop in reverse
-- dependency order, then the enum types, then the extension — a clean full
-- reversal for local dev resets (`sqlx migrate revert`).

DROP TABLE IF EXISTS indexer_checkpoints;
DROP TABLE IF EXISTS audit_log_index;
DROP TABLE IF EXISTS notifications;
DROP TABLE IF EXISTS notification_channels;
DROP TABLE IF EXISTS device_readings_index;
DROP TABLE IF EXISTS device_registrations;
DROP TABLE IF EXISTS record_index;
DROP TABLE IF EXISTS consent_grants;
DROP TABLE IF EXISTS access_requests;
DROP TABLE IF EXISTS provider_staff;
DROP TABLE IF EXISTS providers;
DROP TABLE IF EXISTS patients;

DROP TYPE IF EXISTS audit_event_type;
DROP TYPE IF EXISTS notification_status;
DROP TYPE IF EXISTS notification_channel;
DROP TYPE IF EXISTS notification_event_type;
DROP TYPE IF EXISTS device_status;
DROP TYPE IF EXISTS device_type;
DROP TYPE IF EXISTS storage_backend;
DROP TYPE IF EXISTS purpose_code;
DROP TYPE IF EXISTS record_category;
DROP TYPE IF EXISTS access_request_status;
DROP TYPE IF EXISTS staff_status;
DROP TYPE IF EXISTS staff_role;
DROP TYPE IF EXISTS verification_status;
DROP TYPE IF EXISTS provider_type;
DROP TYPE IF EXISTS patient_status;

DROP EXTENSION IF EXISTS citext;
