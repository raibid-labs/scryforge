# YouTube OAuth Setup Guide

This guide walks you through setting up Google OAuth to access your YouTube data in Scryforge.

## Prerequisites

- Scryforge built and ready
- Sigilforge built and ready
- A Google account with YouTube data

## Step 1: Create Google Cloud Project

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Click "Select a project" → "New Project"
3. Name it (e.g., "Scryforge") and create

## Step 2: Enable YouTube Data API

1. Go to "APIs & Services" → "Library"
2. Search for "YouTube Data API v3"
3. Click "Enable"

## Step 3: Configure OAuth Consent Screen

1. Go to "APIs & Services" → "OAuth consent screen"
2. Choose "External" (or "Internal" for Workspace)
3. Fill in required fields:
   - App name: "Scryforge"
   - User support email: your email
   - Developer contact: your email
4. Click "Save and Continue"
5. Add scopes: Click "Add or Remove Scopes"
   - Find and add: `https://www.googleapis.com/auth/youtube.readonly`
6. Add test users: Add your Google email
7. Complete the wizard

## Step 4: Create OAuth Credentials

1. Go to "APIs & Services" → "Credentials"
2. Click "Create Credentials" → "OAuth client ID"
3. Application type: **Desktop app**
4. Name: "Scryforge CLI"
5. Click "Create"
6. **Copy the Client ID and Client Secret** (you'll need these)

## Step 5: Authenticate with Sigilforge

```bash
cd ~/raibid-labs/sigilforge

# Build if not already built
cargo build --release

# Run authentication (opens browser)
GOOGLE_CLIENT_ID="YOUR_CLIENT_ID.apps.googleusercontent.com" \
GOOGLE_CLIENT_SECRET="YOUR_CLIENT_SECRET" \
./target/release/sigilforge add-account google personal \
  --scopes "https://www.googleapis.com/auth/youtube.readonly"
```

This will:
1. Open your browser to Google's authorization page
2. After you approve, redirect to `localhost:8484`
3. Exchange the code for tokens
4. Store tokens securely in your OS keyring

## Step 6: Verify Authentication

```bash
# List configured accounts
./target/release/sigilforge list-accounts

# Test token retrieval
./target/release/sigilforge get-token google personal
```

## Step 7: Run Scryforge

```bash
cd ~/raibid-labs/scryforge

# Start the daemon (detects sigilforge tokens automatically)
cargo run -p scryforge-daemon --bin scryforge-daemon &

# Start the TUI
cargo run -p scryforge-tui --bin scryforge-tui
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GOOGLE_CLIENT_ID` | OAuth client ID | Required |
| `GOOGLE_CLIENT_SECRET` | OAuth client secret | Optional (recommended) |
| `OAUTH_CALLBACK_PORT` | Local callback port | `8484` |

## Troubleshooting

### "Access blocked: This app's request is invalid"
- Ensure you've added yourself as a test user in OAuth consent screen
- Verify the redirect URI matches `http://127.0.0.1:8484/callback`

### "Token expired"
```bash
# Re-authenticate
sigilforge add-account google personal --scopes "..."
```

### "Keyring unavailable"
- On Linux, ensure `gnome-keyring` or `kwallet` is running
- On headless systems, you may need to configure a keyring daemon

### Port 8484 in use
```bash
OAUTH_CALLBACK_PORT=9999 sigilforge add-account google personal --scopes "..."
```

## Using Without OAuth (Demo Mode)

If you just want to test the TUI without setting up OAuth, the dummy provider provides realistic sample YouTube data:

```bash
cd ~/raibid-labs/scryforge
cargo run -p scryforge-daemon --bin scryforge-daemon &
cargo run -p scryforge-tui --bin scryforge-tui
```

The dummy provider shows:
- Sample subscriptions feed
- Watch Later playlist
- Liked Videos collection
