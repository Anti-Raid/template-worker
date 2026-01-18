CREATE TABLE template_shop_listings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    short TEXT NOT NULL,
    long TEXT NOT NULL,
    review_state TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'approved', 'denied'
    default_allowed_caps TEXT[] NOT NULL DEFAULT '{}'::text[],
    
    -- Content VFS
    language TEXT NOT NULL DEFAULT 'luau',
    content JSONB NOT NULL, -- The actual template content

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);