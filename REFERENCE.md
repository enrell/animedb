# API Reference

Base URL defaults to `http://localhost:8081`. Run the server with:

```bash
go run ./cmd/api :8081
```

(Optional flags `--admin-dsn`, `--anilist-dsn`, `--myanimelist-dsn`, `--videos-dsn` for custom Postgres connections.)

## Table of Contents

1. [Health](#health)
2. [Metadata API (AniList & MyAnimeList)](./REFERENCE_METADATA.md)
3. [Error Responses](#error-responses)
4. [Rate Limiting](#rate-limiting)
5. [Pagination](#pagination)

## Health

| Method | Path      | Description      |
|--------|-----------|------------------|
| GET    | `/healthz` | Returns `{ "status": "ok" }` when the service is ready. |

**Response:**
```jsonc
{
  "status": "ok"
}
```

## Error Responses

All errors are returned as JSON with an HTTP status code and a message:

```jsonc
{
  "error": "not found"
}
```

### HTTP Status Codes

| Code | Description |
|------|-------------|
| `200 OK` | Successful request |
| `201 Created` | Resource created successfully |
| `202 Accepted` | Request accepted for processing (async operations) |
| `204 No Content` | Successful deletion or update with no content |
| `400 Bad Request` | Invalid request parameters or body |
| `404 Not Found` | Resource not found |
| `409 Conflict` | Resource conflict (e.g., duplicate entry, operation in progress) |
| `416 Range Not Satisfiable` | Invalid byte range request |
| `429 Too Many Requests` | Rate limit exceeded |
| `500 Internal Server Error` | Server error |
| `503 Service Unavailable` | Service temporarily unavailable |

### Error Examples

**400 Bad Request:**
```jsonc
{
  "error": "path parameter is required"
}
```

**404 Not Found:**
```jsonc
{
  "error": "not found"
}
```

**409 Conflict:**
```jsonc
{
  "error": "scan already in progress"
}
```

**500 Internal Server Error:**
```jsonc
{
  "error": "database connection failed"
}
```

## Rate Limiting

The API implements rate limiting to prevent abuse. By default, the rate limit is 100 requests per minute per IP address.

When rate limited, the API returns:
- Status Code: `429 Too Many Requests`
- Header: `Retry-After: 60`
- Body:
```jsonc
{
  "error": "rate limit exceeded, retry after 60 seconds"
}
```

## Pagination

List endpoints support pagination with the following query parameters:

| Parameter | Type | Default | Max | Description |
|-----------|------|---------|-----|-------------|
| `page` | int | `1` | – | 1-based page index |
| `page_size` | int | `20` | `500` | Number of items per page |

**Response Format:**
```jsonc
{
  "data": [ /* array of items */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 100,
    "has_more": true
  }
}
```

**Example:**
```bash
curl 'http://localhost:8081/videos/anime?page=2&page_size=50'
```
## Error Responses

All errors are returned as JSON with an HTTP status code and a message:

```jsonc
{
  "error": "not found"
}
```

### HTTP Status Codes

| Code | Description |
|------|-------------|
| `200 OK` | Successful request |
| `201 Created` | Resource created successfully |
| `202 Accepted` | Request accepted for processing (async operations) |
| `204 No Content` | Successful deletion or update with no content |
| `400 Bad Request` | Invalid request parameters or body |
| `404 Not Found` | Resource not found |
| `409 Conflict` | Resource conflict (e.g., duplicate entry, operation in progress) |
| `416 Range Not Satisfiable` | Invalid byte range request |
| `429 Too Many Requests` | Rate limit exceeded |
| `500 Internal Server Error` | Server error |
| `503 Service Unavailable` | Service temporarily unavailable |

### Error Examples

**400 Bad Request:**
```jsonc
{
  "error": "path parameter is required"
}
```

**404 Not Found:**
```jsonc
{
  "error": "not found"
}
```

**409 Conflict:**
```jsonc
{
  "error": "scan already in progress"
}
```

**500 Internal Server Error:**
```jsonc
{
  "error": "database connection failed"
}
```

## Rate Limiting

The API implements rate limiting to prevent abuse. By default, the rate limit is 100 requests per minute per IP address.

When rate limited, the API returns:
- Status Code: `429 Too Many Requests`
- Header: `Retry-After: 60`
- Body:
```jsonc
{
  "error": "rate limit exceeded, retry after 60 seconds"
}
```

## Pagination

List endpoints support pagination with the following query parameters:

| Parameter | Type | Default | Max | Description |
|-----------|------|---------|-----|-------------|
| `page` | int | `1` | – | 1-based page index |
| `page_size` | int | `20` | `500` | Number of items per page |

**Response Format:**
```jsonc
{
  "data": [ /* array of items */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 100,
    "has_more": true
  }
}
```

**Example:**
```bash
curl 'http://localhost:8081/videos/anime?page=2&page_size=50'
```
