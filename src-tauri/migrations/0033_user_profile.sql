CREATE TABLE IF NOT EXISTS user_profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    name TEXT NOT NULL DEFAULT 'User',
    role TEXT,
    bio TEXT,
    avatar_url TEXT,
    email TEXT,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Insert initial empty profile
INSERT OR IGNORE INTO user_profile (id, name) VALUES (1, 'User');
