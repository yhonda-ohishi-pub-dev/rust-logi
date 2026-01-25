-- Add columns for mp4 storage to dvr_notifications table

ALTER TABLE dvr_notifications
ADD COLUMN gcs_key TEXT,
ADD COLUMN file_size_bytes BIGINT,
ADD COLUMN download_status VARCHAR(20) DEFAULT 'pending';

-- Index for querying by download status (useful for retry logic)
CREATE INDEX idx_dvr_notifications_download_status
ON dvr_notifications(organization_id, download_status);
