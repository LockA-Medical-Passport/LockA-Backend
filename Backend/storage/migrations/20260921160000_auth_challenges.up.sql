-- Only challenge transaction hashes are persisted, never signatures or seeds.
CREATE TABLE auth_challenges (
    transaction_hash BYTEA PRIMARY KEY CHECK (octet_length(transaction_hash) = 32),
    stellar_account_id TEXT NOT NULL
        CHECK (char_length(stellar_account_id) = 56 AND stellar_account_id LIKE 'G%'),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ
);
CREATE INDEX auth_challenges_expiry_idx ON auth_challenges (expires_at);
