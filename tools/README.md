# API Tools

Command-line tools for interacting with the Beerio Kart API. Each tool does one thing and saves its state (access token, refresh cookie) so the next tool can pick it up.

## Prerequisites

- The server must be running (`cd backend && cargo run`)
- `curl` (pre-installed on most systems)
- `jq` (optional — pretty-prints JSON output; without it you'll see raw JSON)

## Tools

| Tool | Usage |
|------|-------|
| `register` | `./tools/register <username> <password>` |
| `login` | `./tools/login <username> <password>` |
| `refresh` | `./tools/refresh` |
| `logout` | `./tools/logout` |
| `change-password` | `./tools/change-password <current> <new>` |
| `clear-state` | `./tools/clear-state` |

## How state works

When you `register` or `login`, the tool saves two things to `tools/.state/` (gitignored):

- **Access token** — used by `logout` and `change-password` for the `Authorization: Bearer` header
- **Cookie jar** — holds the refresh token cookie, used by `refresh`

This means you don't have to copy-paste tokens between commands. Each tool reads from and writes to the same state, so they compose naturally.

`logout` and `clear-state` both wipe the saved state. The difference is that `logout` calls the server (revoking all refresh tokens) while `clear-state` just deletes the local files.

## Configuration

By default, tools point at `http://localhost:3000`. To use a different server:

```bash
export BASE_URL=http://192.168.1.50:3000
./tools/login alice password123
```

The env var persists for your terminal session, so you only need to set it once.

## Examples

### Register and log in

```bash
$ ./tools/register alice password123
{
  "access_token": "eyJ...",
  "user": {
    "id": "550e8400-...",
    "username": "alice"
  }
}
HTTP status: 201
```

You're now authenticated. Other tools will use the saved token automatically.

### Refresh the access token

```bash
$ ./tools/refresh
{
  "access_token": "eyJ..."
}
HTTP status: 200
```

### Change your password

```bash
$ ./tools/change-password password123 newpassword456
{
  "access_token": "eyJ..."
}
HTTP status: 200
```

This saves the new access token, so subsequent commands continue to work.

### Log out

```bash
$ ./tools/logout
HTTP/1.1 200 OK
set-cookie: refresh_token=; HttpOnly; ...Max-Age=0
...

(Local token and cookie cleared)
```

### Testing error responses

The tools show the HTTP status code on every response, so you can verify errors by intentionally triggering them:

```bash
# Duplicate username (409)
$ ./tools/register alice password123
$ ./tools/register alice password123
{ "error": "Username already taken" }
HTTP status: 409

# Wrong password (401)
$ ./tools/login alice wrongpassword
{ "error": "Invalid username or password" }
HTTP status: 401

# Refresh without logging in first
$ ./tools/clear-state
$ ./tools/refresh
{ "error": "Missing refresh token" }
HTTP status: 401

# Refresh after logout (revoked)
$ ./tools/login alice password123
$ ./tools/logout
$ ./tools/refresh
{ "error": "Refresh token has been revoked" }
HTTP status: 401

# Password too short (400)
$ ./tools/login alice password123
$ ./tools/change-password password123 short
{ "error": "New password must be 8-128 characters" }
HTTP status: 400
```
