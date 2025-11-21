# API Reference

Base URL defaults to `http://localhost:8081`. Run the server with:

```bash
go run ./cmd/api :8081
```

(Optional flags `--admin-dsn`, `--anilist-dsn`, `--myanimelist-dsn`, `--videos-dsn` for custom Postgres connections.)

## Table of Contents

1. [Health](#health)
2. [Realtime Search](#realtime-search)
3. [AniList Catalog](#anilist-catalog)
4. [MyAnimeList Catalog](#myanimelist-catalog)
5. [Video Catalog](#video-catalog)
   - [Anime Management](#anime-management)
   - [Episode Management](#episode-management)
   - [Search](#video-search)
   - [Scanning](#scanning)
   - [Library Management](#library-management)
   - [Settings Management](#settings-management)
   - [Video Streaming](#video-streaming)
   - [Transcoding](#transcoding)
   - [Hardware Detection](#hardware-detection)

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

## Realtime Search

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/search/realtime` | Global unified search across both AniList and MyAnimeList with configurable source filtering. Returns top K results ranked by BM25. |

### `/search/realtime` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `q`      | string | –       | **Required.** Search query. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |
| `source` | string | `both`  | Filter by source: `anilist`, `myanimelist`, or `both`. |

**Response:**
```jsonc
[
  {
    "id": 1535,
    "title": "Tensei Shitara Slime Datta Ken",
    "romaji": "Tensei Shitara Slime Datta Ken",
    "english": "That Time I Got Reincarnated as a Slime",
    "native": "転生したらスライムだった件",
    "source": "anilist",
    "score": 0.84
  }
]
```

**Example:**
```bash
curl 'http://localhost:8081/search/realtime?q=attack%20titan&source=both&limit=10'
```

## AniList Catalog

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/anilist/media/search` | Returns top K AniList matches ranked by BM25 with n-gram tokenization and season awareness. |
| GET    | `/anilist/media`    | Paginated list of AniList media (anime/manga) stored in Postgres. |
| GET    | `/anilist/media/{id}` | Detailed record for a specific AniList media entry. |

### `/anilist/media` Query Parameters

| Name         | Type   | Default | Notes |
|--------------|--------|---------|-------|
| `page`       | int    | `1`     | 1-based page index. |
| `page_size`  | int    | `20`    | Max `500`. |
| `search`     | string | –       | Fuzzy match (trigram prefilter → BM25 re-rank) against title fields. |
| `title_romaji` | string | –     | Case-insensitive partial match on the romaji title column. |
| `title_english` | string | –    | Case-insensitive partial match on the English title column. |
| `title_native` | string | –     | Case-insensitive partial match on the native title column. |
| `type`       | string | –       | Exact match on `type` column (e.g., `ANIME`, `MANGA`). |
| `season`     | string | –       | Exact match on `season` (accepted values upper-cased internally). |
| `season_year`| int    | –       | Filters by release year. |

**Response:**
```jsonc
{
  "data": [ /* array of AniList media records */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 1234,
    "has_more": true
  }
}
```

### `/anilist/media/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | AniList media identifier. |

### `/anilist/media/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `search` | string | –       | **Required.** Search query. Returns up to K results with a `score` field. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |

**Response:**
```jsonc
[
  {
    "id": 1535,
    "title": "Tensei Shitara Slime Datta Ken",
    "romaji": "Tensei Shitara Slime Datta Ken",
    "english": "That Time I Got Reincarnated as a Slime",
    "native": "転生したらスライムだった件",
    "score": 0.84
  }
]
```

**Examples:**
```bash
# Search AniList (top 5 results)
curl 'http://localhost:8081/anilist/media/search?search=slime&limit=5'

# Paginated list with filters
curl 'http://localhost:8081/anilist/media?page=1&page_size=50&type=ANIME&season=WINTER&season_year=2024'

# Get specific media
curl 'http://localhost:8081/anilist/media/1535'
```

## MyAnimeList Catalog

| Method | Path                   | Description |
|--------|------------------------|-------------|
| GET    | `/myanimelist/anime/search` | Returns top K MyAnimeList matches ranked by BM25. |
| GET    | `/myanimelist/anime`   | Paginated list of anime fetched via the Jikan API and stored locally. |
| GET    | `/myanimelist/anime/{id}` | Detailed record for a specific MyAnimeList entry. |

### `/myanimelist/anime` Query Parameters

| Name        | Type   | Default | Notes |
|-------------|--------|---------|-------|
| `page`      | int    | `1`     | 1-based page index. |
| `page_size` | int    | `20`    | Max `500`. |
| `search`    | string | –       | Fuzzy match (trigram prefilter → BM25 re-rank) across title fields. |
| `type`      | string | –       | Exact match on `type` column (e.g., `TV`, `Movie`). |
| `season`    | string | –       | Filters by season string (stored lowercase). |
| `year`      | int    | –       | Filters by release year. |

**Response:**
```jsonc
{
  "data": [ /* array of MyAnimeList anime records */ ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 5678,
    "has_more": true
  }
}
```

### `/myanimelist/anime/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | MyAnimeList `mal_id`. |

### `/myanimelist/anime/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `search` | string | –       | **Required.** Search query. Returns up to K results with a `score` field. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |

**Response:**
```jsonc
[
  {
    "id": 39535,
    "title": "Tensei shitara Slime Datta Ken",
    "score": 0.82
  }
]
```

**Examples:**
```bash
# Search MyAnimeList
curl 'http://localhost:8081/myanimelist/anime/search?search=attack%20titan&limit=10'

# Paginated list
curl 'http://localhost:8081/myanimelist/anime?page=2&page_size=50&type=TV&year=2023'

# Get specific anime
curl 'http://localhost:8081/myanimelist/anime/39535'
```

## Video Catalog

The Video Catalog API provides endpoints for managing locally stored video files, including anime series, episodes, thumbnails, libraries, settings, and streaming capabilities.

## Anime Management

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/anime`     | Paginated list of anime series indexed from local video files. |
| GET    | `/videos/anime/{id}` | Detailed record for a specific anime series. |

### `/videos/anime` Query Parameters

| Name         | Type   | Default | Notes |
|--------------|--------|---------|-------|
| `page`       | int    | `1`     | 1-based page index. |
| `page_size`  | int    | `20`    | Max `500`. |
| `search`     | string | –       | Fuzzy match against anime title. |

**Response:**
```jsonc
{
  "data": [
    {
      "id": 1,
      "title": "Attack on Titan",
      "folder_path": "/videos/anime/aot",
      "created_at": "2025-10-31T05:00:00Z",
      "updated_at": "2025-10-31T05:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 100,
    "has_more": true
  }
}
```

### `/videos/anime/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | Anime series identifier. |

**Response:**
```jsonc
{
  "id": 1,
  "title": "Attack on Titan",
  "folder_path": "/videos/anime/aot",
  "created_at": "2025-10-31T05:00:00Z",
  "updated_at": "2025-10-31T05:00:00Z"
}
```

**Examples:**
```bash
# List all anime
curl 'http://localhost:8081/videos/anime?page=1&page_size=20'

# Search anime
curl 'http://localhost:8081/videos/anime?search=slime'

# Get specific anime
curl 'http://localhost:8081/videos/anime/1'
```

## Episode Management

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/anime/{id}/episodes` | List all episodes for an anime series. |
| GET    | `/videos/episodes/{id}` | Detailed record for a specific episode. |
| GET    | `/videos/episodes/{id}/thumbnails` | List thumbnails extracted for an episode. |

### `/videos/anime/{id}/episodes` Path and Query Parameters

| Name         | Type   | Default | Notes |
|--------------|--------|---------|-------|
| `id`         | int    | –       | **Required.** Anime series identifier. |
| `page`       | int    | `1`     | 1-based page index. |
| `page_size`  | int    | `20`    | Max `500`. |

**Response:**
```jsonc
{
  "data": [
    {
      "id": 1,
      "anime_id": 1,
      "file_path": "/videos/anime/aot/ep01.mkv",
      "filename": "ep01.mkv",
      "file_size": 1048576000,
      "duration": 1440.5,
      "hash": "",
      "format": "matroska",
      "resolution": "1920x1080",
      "episode_number": 1,
      "season_number": 1,
      "is_corrupted": false,
      "is_partial": false,
      "created_at": "2025-10-31T05:00:00Z",
      "updated_at": "2025-10-31T05:00:00Z",
      "indexed_at": "2025-10-31T05:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "page_size": 20,
    "total": 25,
    "has_more": false
  }
}
```

### `/videos/episodes/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | Episode identifier. |

**Response:**
```jsonc
{
  "id": 1,
  "anime_id": 1,
  "file_path": "/videos/anime/aot/ep01.mkv",
  "filename": "ep01.mkv",
  "file_size": 1048576000,
  "duration": 1440.5,
  "hash": "",
  "format": "matroska",
  "resolution": "1920x1080",
  "episode_number": 1,
  "season_number": 1,
  "is_corrupted": false,
  "is_partial": false,
  "created_at": "2025-10-31T05:00:00Z",
  "updated_at": "2025-10-31T05:00:00Z",
  "indexed_at": "2025-10-31T05:00:00Z"
}
```

### `/videos/episodes/{id}/thumbnails` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | Episode identifier. |

**Response:**
```jsonc
[
  {
    "id": 1,
    "episode_id": 1,
    "file_path": "/videos/anime/aot/thumbnails/ep01_120.0.jpg",
    "timestamp_sec": 120.0,
    "created_at": "2025-10-31T05:00:00Z"
  }
]
```

**Examples:**
```bash
# List episodes for anime
curl 'http://localhost:8081/videos/anime/1/episodes'

# Get specific episode
curl 'http://localhost:8081/videos/episodes/1'

# Get episode thumbnails
curl 'http://localhost:8081/videos/episodes/1/thumbnails'
```

## Video Search

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/search`    | Search anime by title using BM25 ranking algorithm. |

### `/videos/search` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `q`      | string | –       | **Required.** Search query. |
| `limit`  | int    | `10`    | Max results to return. Capped at 50. |

**Response:**
```jsonc
[
  {
    "id": 1,
    "title": "Attack on Titan",
    "folder_path": "/videos/anime/aot",
    "score": 0.92
  }
]
```

**Example:**
```bash
curl 'http://localhost:8081/videos/search?q=attack%20titan&limit=10'
```

## Scanning

| Method | Path                | Description |
|--------|---------------------|-------------|
| POST   | `/videos/scan`      | Trigger manual scan of video files (async). |
| GET    | `/videos/scan/status` | Get current scan status. |

### `/videos/scan` Query Parameters

| Name     | Type   | Default | Notes |
|----------|--------|---------|-------|
| `path`   | string | –       | **Required.** Directory to scan for video files. |

**Request:**
```bash
curl -X POST 'http://localhost:8081/videos/scan?path=/home/user/animes'
```

**Response:**
```jsonc
{
  "status": "scan started",
  "path": "/home/user/animes"
}
```

**Status Codes:**
- `202 Accepted` - Scan started successfully
- `400 Bad Request` - Path parameter missing
- `409 Conflict` - Scan already in progress

### `/videos/scan/status` Response

**Response:**
```jsonc
{
  "status": "scanning"
}
```

**Possible values:**
- `idle` - No scan in progress
- `scanning` - Scan currently running

**Example:**
```bash
curl 'http://localhost:8081/videos/scan/status'
```

## Library Management

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/libraries` | List all library paths configured for watching. |
| POST   | `/videos/libraries` | Create a new library path. |
| GET    | `/videos/libraries/{id}` | Get library details. |
| PUT    | `/videos/libraries/{id}` | Update library name. |
| DELETE | `/videos/libraries/{id}` | Delete a library. |

### `/videos/libraries` GET Response

**Response:**
```jsonc
[
  {
    "id": 1,
    "path": "/home/user/animes",
    "name": "Main Library",
    "created_at": "2025-10-31T05:00:00Z",
    "updated_at": "2025-10-31T05:00:00Z"
  }
]
```

### `/videos/libraries` POST Request Body

**Request:**
```jsonc
{
  "path": "/home/user/animes",
  "name": "Main Library"
}
```

**Response:**
```jsonc
{
  "id": 1,
  "path": "/home/user/animes",
  "name": "Main Library",
  "created_at": "2025-10-31T05:00:00Z",
  "updated_at": "2025-10-31T05:00:00Z"
}
```

**Status Codes:**
- `201 Created` - Library created successfully
- `400 Bad Request` - Invalid request body or missing path
- `409 Conflict` - Library path already exists
- `500 Internal Server Error` - Server error

### `/videos/libraries/{id}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `id` | int  | Library identifier. |

### `/videos/libraries/{id}` PUT Request Body

**Request:**
```jsonc
{
  "name": "Updated Library Name"
}
```

**Response:**
```jsonc
{
  "id": 1,
  "path": "/home/user/animes",
  "name": "Updated Library Name",
  "created_at": "2025-10-31T05:00:00Z",
  "updated_at": "2025-10-31T05:00:00Z"
}
```

### `/videos/libraries/{id}` DELETE Response

**Status Codes:**
- `204 No Content` - Library deleted successfully
- `404 Not Found` - Library not found
- `500 Internal Server Error` - Server error

**Examples:**
```bash
# List all libraries
curl 'http://localhost:8081/videos/libraries'

# Create library
curl -X POST 'http://localhost:8081/videos/libraries' \
  -H "Content-Type: application/json" \
  -d '{"path": "/home/user/animes", "name": "Main Library"}'

# Get library
curl 'http://localhost:8081/videos/libraries/1'

# Update library
curl -X PUT 'http://localhost:8081/videos/libraries/1' \
  -H "Content-Type: application/json" \
  -d '{"name": "Updated Name"}'

# Delete library
curl -X DELETE 'http://localhost:8081/videos/libraries/1'
```

## Settings Management

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/settings`  | Get all settings as key-value map. |
| PUT    | `/videos/settings`  | Update multiple settings at once. |
| GET    | `/videos/settings/{key}` | Get a specific setting by key. |
| DELETE | `/videos/settings/{key}` | Delete a setting. |

### `/videos/settings` GET Response

**Response:**
```jsonc
{
  "subtitle_language": "en",
  "default_player": "mpv",
  "auto_play": "true",
  "transcode_enabled": "false",
  "transcode_preset": "fast"
}
```

### `/videos/settings` PUT Request Body

**Request:**
```jsonc
{
  "subtitle_language": "en",
  "default_player": "mpv",
  "auto_play": "true",
  "transcode_enabled": "true",
  "transcode_hardware_encoder": "auto",
  "transcode_preset": "fast",
  "transcode_resolution": "720",
  "transcode_container": "mp4"
}
```

**Response:**
```jsonc
{
  "subtitle_language": "en",
  "default_player": "mpv",
  "auto_play": "true",
  "transcode_enabled": "true",
  "transcode_hardware_encoder": "auto",
  "transcode_preset": "fast",
  "transcode_resolution": "720",
  "transcode_container": "mp4"
}
```

### `/videos/settings/{key}` Path Parameter

| Name | Type | Description |
|------|------|-------------|
| `key` | string  | Setting key name. |

**Response:**
```jsonc
{
  "id": 1,
  "key": "subtitle_language",
  "value": "en",
  "created_at": "2025-10-31T05:00:00Z",
  "updated_at": "2025-10-31T05:00:00Z"
}
```

### Available Settings

#### General Settings

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `subtitle_language` | string | – | Preferred subtitle language (e.g., `en`, `ja`, `pt`) |
| `default_player` | string | – | Default video player (e.g., `mpv`, `vlc`, `potplayer`) |
| `auto_play` | bool | `false` | Auto-play next episode |

#### Transcoding Settings

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `transcode_enabled` | bool | `false` | Enable transcoding by default |
| `transcode_hardware_encoder` | string | `auto` | Hardware encoder: `auto`, `h264_nvenc`, `h264_qsv`, `h264_amf`, `libx264` |
| `transcode_hardware_acceleration` | string | `auto` | Hardware acceleration: `cuda`, `qsv`, `d3d11va` |
| `transcode_preset` | string | `fast` | Encoding preset: `ultrafast`, `superfast`, `veryfast`, `faster`, `fast`, `medium`, `slow`, `slower`, `veryslow` |
| `transcode_resolution` | string | `""` | Output resolution (e.g., `720`, `1080`, `-1:720`) |
| `transcode_video_codec` | string | `""` | Video codec (auto-detect if empty) |
| `transcode_audio_codec` | string | `aac` | Audio codec (default: `aac`) |
| `transcode_tune` | string | `""` | Tune settings (e.g., `zerolatency` for streaming) |
| `transcode_container` | string | `mp4` | Output container: `mp4`, `webm`, `mkv` |
| `transcode_remux_only` | bool | `false` | Only remux (container change) without re-encoding |

**Examples:**
```bash
# Get all settings
curl 'http://localhost:8081/videos/settings'

# Update multiple settings
curl -X PUT 'http://localhost:8081/videos/settings' \
  -H "Content-Type: application/json" \
  -d '{
    "subtitle_language": "en",
    "default_player": "mpv",
    "transcode_enabled": "true",
    "transcode_preset": "fast"
  }'

# Get specific setting
curl 'http://localhost:8081/videos/settings/subtitle_language'

# Delete setting
curl -X DELETE 'http://localhost:8081/videos/settings/auto_play'
```

## Video Streaming

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/episodes/{id}/stream` | Stream video file with HTTP Range Request support (RFC 7233). |

### `/videos/episodes/{id}/stream` Path and Query Parameters

| Name         | Type   | Default | Notes |
|--------------|--------|---------|-------|
| `id`         | int    | –       | **Required.** Episode identifier. |
| `transcode`  | string | `false` | Set to `true` or `1` to enable transcoding. |

### HTTP Range Request Support

This endpoint supports full HTTP Range Request (RFC 7233) functionality, enabling:
- **Seek support**: Clients can request specific byte ranges
- **Partial content**: Efficient streaming of video segments
- **Resume playback**: Clients can resume interrupted downloads
- **Adaptive bitrate**: Compatible with adaptive streaming protocols

**Headers:**
- `Range: bytes=start-end` - Request a specific byte range
- `Accept-Ranges: bytes` - Server indicates range support
- `Content-Range: bytes start-end/total` - Server returns range information

**Response Status Codes:**
- `200 OK` - Full file response (no Range header)
- `206 Partial Content` - Partial file response (Range header present)
- `416 Range Not Satisfiable` - Invalid range request

**Supported formats:**
- MP4 (video/mp4)
- MKV (video/x-matroska)
- AVI (video/x-msvideo)
- WebM (video/webm)
- MOV (video/quicktime)

The endpoint automatically detects the content type based on file extension and sets appropriate HTTP headers.

**Examples:**
```bash
# Full video download
curl 'http://localhost:8081/videos/episodes/1/stream' --output video.mp4

# Request specific byte range (first 1MB)
curl -H "Range: bytes=0-1048575" 'http://localhost:8081/videos/episodes/1/stream'

# Request from middle of file (bytes 100MB-200MB)
curl -H "Range: bytes=104857600-209715199" 'http://localhost:8081/videos/episodes/1/stream'

# Stream with transcoding
curl 'http://localhost:8081/videos/episodes/1/stream?transcode=true' --output transcoded.mp4
```

## Transcoding

The streaming endpoint supports on-demand transcoding with hardware acceleration when enabled via settings or the `transcode` query parameter.

### Transcoding Strategy

1. **Remux first**: If the file can be remuxed (container change only), it uses fast remuxing without re-encoding
2. **Hardware acceleration**: Auto-detects and uses GPU encoders (NVIDIA NVENC, Intel QSV, AMD AMF)
3. **Software fallback**: Falls back to CPU encoding (`libx264`) if hardware is unavailable
4. **Caching**: Transcoded files are cached to avoid re-transcoding

### Transcoding Settings

See [Settings Management](#settings-management) for all available transcoding settings.

**Quick Configuration:**
```bash
curl -X PUT 'http://localhost:8081/videos/settings' \
  -H "Content-Type: application/json" \
  -d '{
    "transcode_enabled": "true",
    "transcode_hardware_encoder": "auto",
    "transcode_preset": "fast",
    "transcode_resolution": "720",
    "transcode_container": "mp4",
    "transcode_tune": "zerolatency"
  }'
```

**Usage:**
```bash
# Stream with transcoding (uses settings or query parameter)
curl 'http://localhost:8081/videos/episodes/1/stream?transcode=true'
```

## Hardware Detection

| Method | Path                | Description |
|--------|---------------------|-------------|
| GET    | `/videos/hardware` | Get hardware acceleration information and available encoders. |

### `/videos/hardware` Response

**Response:**
```jsonc
{
  "has_hardware": true,
  "default_encoder": "h264_nvenc",
  "default_accel": "cuda",
  "available_encoders": {
    "h264_nvenc": true,
    "hevc_nvenc": true
  },
  "nvidia": {
    "name": "NVIDIA NVENC",
    "encoder": "h264_nvenc",
    "decoder": "h264_cuvid",
    "acceleration": "cuda",
    "available": true
  },
  "intel": {
    "name": "Intel Quick Sync",
    "encoder": "h264_qsv",
    "decoder": "h264_qsv",
    "acceleration": "qsv",
    "available": true
  },
  "amd": {
    "name": "AMD AMF",
    "encoder": "h264_amf",
    "decoder": "h264_amf",
    "acceleration": "d3d11va",
    "available": false
  },
  "best_encoder": {
    "name": "NVIDIA NVENC",
    "encoder": "h264_nvenc",
    "decoder": "h264_cuvid",
    "acceleration": "cuda"
  }
}
```

**Example:**
```bash
curl 'http://localhost:8081/videos/hardware'
```

**Response Fields:**
- `has_hardware` - Boolean indicating if any hardware encoder is available
- `default_encoder` - Recommended encoder (best available hardware or libx264)
- `default_accel` - Recommended hardware acceleration method
- `available_encoders` - Map of available encoder names
- `nvidia` - NVIDIA NVENC information (if available)
- `intel` - Intel Quick Sync Video information (if available)
- `amd` - AMD AMF information (if available)
- `best_encoder` - Recommended encoder configuration

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
