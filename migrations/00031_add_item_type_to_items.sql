-- Migration: Add item_type column to items table
-- Distinguishes folders from items explicitly (replaces frontend heuristic)

ALTER TABLE items ADD COLUMN item_type TEXT NOT NULL DEFAULT 'item';

-- Existing data: items with no barcode, image_url, or url are folders
UPDATE items SET item_type = 'folder'
WHERE barcode IS NULL AND image_url IS NULL AND url IS NULL;
