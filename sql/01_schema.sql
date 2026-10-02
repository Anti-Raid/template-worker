DO $$ BEGIN
    CREATE TYPE GUILD_CHANNEL_TYPE AS ENUM ('audit_logs');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS cmd_usage_stats (
    -- Name or identifier of the executed command
    command_name TEXT PRIMARY KEY,
    -- Total number of times this command has been invoked
    uses INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS global_kv (
    -- Primary lookup key within the global key-value store
    key TEXT NOT NULL,
    -- Monotonically increasing version number for the key
    version INTEGER NOT NULL,
    -- Partitioning namespace or scope for the key-value record
    scope TEXT NOT NULL,
    -- Snowflake or account ID of the entity that owns this entry
    owner_id TEXT NOT NULL,
    -- Entity discriminator for the owner (e.g. staff, user, guild)
    owner_type TEXT NOT NULL,
    -- Publicly accessible JSON metadata describing the entry
    public_metadata JSONB NOT NULL,
    -- Main JSON payload data stored for this entry
    data JSONB NOT NULL,
    -- Whether the main payload is publicly readable without authorization
    public_data BOOLEAN NOT NULL DEFAULT false,
    -- Timestamp when the global key-value record was created
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Timestamp when the global key-value record was last modified
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Optional purchase or access price in credits (staff-only)
    price INTEGER,
    -- Marketplace review status (e.g. pending, approved, rejected)
    review_state TEXT NOT NULL DEFAULT 'pending',
    -- Short description or synopsis of the key-value entry
    short TEXT NOT NULL,
    -- Detailed long-form documentation or description
    long TEXT,
    PRIMARY KEY (key, version, scope),
    CONSTRAINT global_kv_staff_price CHECK (((price IS NULL) OR ((price IS NOT NULL) AND (owner_type = 'staff'::text))))
);

CREATE TABLE IF NOT EXISTS shop_listings_v2 (
    -- Unique UUID identifier for the shop listing
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Display name or title of the shop listing package
    name TEXT NOT NULL,
    -- Brief summary description shown in browse and search views
    short TEXT NOT NULL,
    -- Comprehensive documentation and usage instructions
    long TEXT NOT NULL,
    -- Submission moderation status (e.g. pending, approved, rejected)
    review_state TEXT NOT NULL DEFAULT 'pending',
    -- List of default worker capability permissions granted to this listing
    default_allowed_caps TEXT[] NOT NULL DEFAULT '{}',
    -- Target runtime scripting language (default: luau)
    language TEXT NOT NULL DEFAULT 'luau',
    -- JSON payload containing script source code, assets, or configuration
    content JSONB NOT NULL,
    -- Timestamp when the shop listing was published
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Timestamp when the shop listing was last updated
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Snowflake or account ID of the listing publisher
    owner_id TEXT NOT NULL,
    -- Entity type of the listing owner (e.g. user, guild)
    owner_type TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tenant_kv (
    -- Unique identifier for the tenant key-value entry
    id TEXT PRIMARY KEY,
    -- Discord snowflake ID of the tenant (e.g. guild ID)
    owner_id TEXT NOT NULL,
    -- Key name within the tenant namespace
    key TEXT NOT NULL,
    -- Structured JSON payload for the tenant entry
    value JSONB,
    -- Timestamp when the tenant key-value entry was created
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Timestamp when the tenant key-value entry was last modified
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Tenant discriminator type (defaults to guild)
    owner_type TEXT NOT NULL DEFAULT 'guild',
    -- Optional binary payload data (up to 512 KiB)
    blob BYTEA,
    -- Partitioning scope or domain within the tenant
    scope TEXT NOT NULL,
    CONSTRAINT kv_unique_entry UNIQUE (owner_id, owner_type, key, scope),
    CONSTRAINT enforce_max_blob_size CHECK ((octet_length(blob) <= 524288))
);

CREATE TABLE IF NOT EXISTS tenant_state (
    -- Discord snowflake ID of the tenant owner (e.g. guild ID)
    owner_id TEXT NOT NULL,
    -- Tenant discriminator type (e.g. guild)
    owner_type TEXT NOT NULL,
    -- Whether this tenant is banned from worker execution
    banned BOOLEAN NOT NULL DEFAULT false,
    -- Bitmask integer representing moderation and security feature flags
    modflags INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (owner_id, owner_type)
);

CREATE TABLE IF NOT EXISTS users (
    -- Unique Discord snowflake user ID
    user_id VARCHAR(255) PRIMARY KEY,
    -- Timestamp when the user account was first registered
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Account status or moderation state (e.g. normal, banned)
    state TEXT NOT NULL DEFAULT 'normal',
    -- Timestamp when the user record was last updated
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Whether the user is restricted from voting on bot lists
    vote_banned BOOLEAN NOT NULL DEFAULT false,
    -- Cached JSON array of Discord guilds the user belongs to
    guilds_cache JSONB
);

CREATE TABLE IF NOT EXISTS tenant_state_events (
    -- Discord snowflake ID of the tenant
    owner_id TEXT NOT NULL,
    -- Tenant discriminator type (e.g. guild)
    owner_type TEXT NOT NULL,
    -- Event name or trigger subscription (e.g. guild_member_add)
    event TEXT NOT NULL,
    -- Subsystem or worker service handling this event subscription
    system TEXT NOT NULL,
    PRIMARY KEY (owner_id, owner_type, event, system),
    FOREIGN KEY (owner_id, owner_type) REFERENCES tenant_state(owner_id, owner_type) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_oauths (
    -- Discord snowflake user ID referencing users table
    user_id TEXT PRIMARY KEY,
    -- Discord OAuth2 access token
    access_token TEXT NOT NULL,
    -- Discord OAuth2 refresh token used to renew access
    refresh_token TEXT NOT NULL,
    -- Timestamp when the access token was last updated
    access_token_last_set TIMESTAMPTZ NOT NULL,
    -- Lifespan in seconds of the current access token
    access_token_expiry INTEGER NOT NULL,
    -- Space-separated list of authorized Discord OAuth2 scopes
    scope TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS web_api_tokens (
    -- Secret authentication token string used in HTTP API headers
    token TEXT PRIMARY KEY,
    -- Discord snowflake ID of the owning user
    user_id TEXT NOT NULL,
    -- Timestamp when the API token was generated
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Token category or authorization tier
    type TEXT NOT NULL,
    -- Timestamp after which the token is invalid
    expiry TIMESTAMPTZ NOT NULL,
    -- Unique UUID identifier for managing or revoking this token
    id UUID NOT NULL DEFAULT uuid_generate_v4(),
    -- Optional human-readable label or description for this token
    name TEXT,
    CONSTRAINT web_api_tokens_id_key UNIQUE (id),
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_tenant_events ON tenant_state_events USING btree (owner_id, owner_type, event);

COMMENT ON COLUMN cmd_usage_stats.command_name IS 'Name or identifier of the executed command';
COMMENT ON COLUMN cmd_usage_stats.uses IS 'Total number of times this command has been invoked';
COMMENT ON COLUMN global_kv.key IS 'Primary lookup key within the global key-value store';
COMMENT ON COLUMN global_kv.version IS 'Monotonically increasing version number for the key';
COMMENT ON COLUMN global_kv.scope IS 'Partitioning namespace or scope for the key-value record';
COMMENT ON COLUMN global_kv.owner_id IS 'Snowflake or account ID of the entity that owns this entry';
COMMENT ON COLUMN global_kv.owner_type IS 'Entity discriminator for the owner (e.g. staff, user, guild)';
COMMENT ON COLUMN global_kv.public_metadata IS 'Publicly accessible JSON metadata describing the entry';
COMMENT ON COLUMN global_kv.data IS 'Main JSON payload data stored for this entry';
COMMENT ON COLUMN global_kv.public_data IS 'Whether the main payload is publicly readable without authorization';
COMMENT ON COLUMN global_kv.created_at IS 'Timestamp when the global key-value record was created';
COMMENT ON COLUMN global_kv.last_updated_at IS 'Timestamp when the global key-value record was last modified';
COMMENT ON COLUMN global_kv.price IS 'Optional purchase or access price in credits (staff-only)';
COMMENT ON COLUMN global_kv.review_state IS 'Marketplace review status (e.g. pending, approved, rejected)';
COMMENT ON COLUMN global_kv.short IS 'Short description or synopsis of the key-value entry';
COMMENT ON COLUMN global_kv.long IS 'Detailed long-form documentation or description';
COMMENT ON COLUMN shop_listings_v2.id IS 'Unique UUID identifier for the shop listing';
COMMENT ON COLUMN shop_listings_v2.name IS 'Display name or title of the shop listing package';
COMMENT ON COLUMN shop_listings_v2.short IS 'Brief summary description shown in browse and search views';
COMMENT ON COLUMN shop_listings_v2.long IS 'Comprehensive documentation and usage instructions';
COMMENT ON COLUMN shop_listings_v2.review_state IS 'Submission moderation status (e.g. pending, approved, rejected)';
COMMENT ON COLUMN shop_listings_v2.default_allowed_caps IS 'List of default worker capability permissions granted to this listing';
COMMENT ON COLUMN shop_listings_v2.language IS 'Target runtime scripting language (default: luau)';
COMMENT ON COLUMN shop_listings_v2.content IS 'JSON payload containing script source code, assets, or configuration';
COMMENT ON COLUMN shop_listings_v2.created_at IS 'Timestamp when the shop listing was published';
COMMENT ON COLUMN shop_listings_v2.last_updated_at IS 'Timestamp when the shop listing was last updated';
COMMENT ON COLUMN shop_listings_v2.owner_id IS 'Snowflake or account ID of the listing publisher';
COMMENT ON COLUMN shop_listings_v2.owner_type IS 'Entity type of the listing owner (e.g. user, guild)';
COMMENT ON COLUMN tenant_kv.id IS 'Unique identifier for the tenant key-value entry';
COMMENT ON COLUMN tenant_kv.owner_id IS 'Discord snowflake ID of the tenant (e.g. guild ID)';
COMMENT ON COLUMN tenant_kv.key IS 'Key name within the tenant namespace';
COMMENT ON COLUMN tenant_kv.value IS 'Structured JSON payload for the tenant entry';
COMMENT ON COLUMN tenant_kv.created_at IS 'Timestamp when the tenant key-value entry was created';
COMMENT ON COLUMN tenant_kv.last_updated_at IS 'Timestamp when the tenant key-value entry was last modified';
COMMENT ON COLUMN tenant_kv.owner_type IS 'Tenant discriminator type (defaults to guild)';
COMMENT ON COLUMN tenant_kv.blob IS 'Optional binary payload data (up to 512 KiB)';
COMMENT ON COLUMN tenant_kv.scope IS 'Partitioning scope or domain within the tenant';
COMMENT ON COLUMN tenant_state.owner_id IS 'Discord snowflake ID of the tenant owner (e.g. guild ID)';
COMMENT ON COLUMN tenant_state.owner_type IS 'Tenant discriminator type (e.g. guild)';
COMMENT ON COLUMN tenant_state.banned IS 'Whether this tenant is banned from worker execution';
COMMENT ON COLUMN tenant_state.modflags IS 'Bitmask integer representing moderation and security feature flags';
COMMENT ON COLUMN users.user_id IS 'Unique Discord snowflake user ID';
COMMENT ON COLUMN users.created_at IS 'Timestamp when the user account was first registered';
COMMENT ON COLUMN users.state IS 'Account status or moderation state (e.g. normal, banned)';
COMMENT ON COLUMN users.updated_at IS 'Timestamp when the user record was last updated';
COMMENT ON COLUMN users.vote_banned IS 'Whether the user is restricted from voting on bot lists';
COMMENT ON COLUMN users.guilds_cache IS 'Cached JSON array of Discord guilds the user belongs to';
COMMENT ON COLUMN tenant_state_events.owner_id IS 'Discord snowflake ID of the tenant';
COMMENT ON COLUMN tenant_state_events.owner_type IS 'Tenant discriminator type (e.g. guild)';
COMMENT ON COLUMN tenant_state_events.event IS 'Event name or trigger subscription (e.g. guild_member_add)';
COMMENT ON COLUMN tenant_state_events.system IS 'Subsystem or worker service handling this event subscription';
COMMENT ON COLUMN user_oauths.user_id IS 'Discord snowflake user ID referencing users table';
COMMENT ON COLUMN user_oauths.access_token IS 'Discord OAuth2 access token';
COMMENT ON COLUMN user_oauths.refresh_token IS 'Discord OAuth2 refresh token used to renew access';
COMMENT ON COLUMN user_oauths.access_token_last_set IS 'Timestamp when the access token was last updated';
COMMENT ON COLUMN user_oauths.access_token_expiry IS 'Lifespan in seconds of the current access token';
COMMENT ON COLUMN user_oauths.scope IS 'Space-separated list of authorized Discord OAuth2 scopes';
COMMENT ON COLUMN web_api_tokens.token IS 'Secret authentication token string used in HTTP API headers';
COMMENT ON COLUMN web_api_tokens.user_id IS 'Discord snowflake ID of the owning user';
COMMENT ON COLUMN web_api_tokens.created_at IS 'Timestamp when the API token was generated';
COMMENT ON COLUMN web_api_tokens.type IS 'Token category or authorization tier';
COMMENT ON COLUMN web_api_tokens.expiry IS 'Timestamp after which the token is invalid';
COMMENT ON COLUMN web_api_tokens.id IS 'Unique UUID identifier for managing or revoking this token';
COMMENT ON COLUMN web_api_tokens.name IS 'Optional human-readable label or description for this token';
