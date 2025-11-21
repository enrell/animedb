package handlers

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/go-chi/chi/v5"

	"animedb/internal/http/response"
	"animedb/internal/indexer"
	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/service"
	"animedb/internal/util"
	"animedb/internal/video"
	"animedb/internal/watcher"
)

type VideoHandlers struct {
	repo            repository.VideoRepository
	aniRepo         repository.AniListRepository
	malRepo         repository.MyAnimeListRepository
	searchService   *service.VideoSearchService
	indexer         *indexer.VideoIndexer
	transcodeService *service.TranscodeService
	watcher         watcher.Watcher
	scanMu          sync.Mutex
	scanning        bool
}

func NewVideoHandlers(repo repository.VideoRepository, aniRepo repository.AniListRepository, malRepo repository.MyAnimeListRepository, indexer *indexer.VideoIndexer, transcodeService *service.TranscodeService, fileWatcher watcher.Watcher) *VideoHandlers {
	return &VideoHandlers{
		repo:            repo,
		aniRepo:         aniRepo,
		malRepo:         malRepo,
		searchService:   service.NewVideoSearchService(repo),
		indexer:         indexer,
		transcodeService: transcodeService,
		watcher:         fileWatcher,
	}
}

func (h *VideoHandlers) AnimeList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	page, pageSize := response.ParsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 500)
	search := r.URL.Query().Get("search")

	animeList, total, err := h.repo.ListAnime(ctx, search, page, pageSize)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	for i := range animeList {
		h.ensureAniListMetadata(ctx, &animeList[i])
	}

	response.WriteJSON(w, http.StatusOK, response.ListResponse[model.Anime]{
		Data: animeList,
		Pagination: response.PaginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
			HasMore:  page*pageSize < total,
		},
	})
}

func (h *VideoHandlers) AnimeGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	anime, err := h.repo.GetAnimeByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	h.ensureAniListMetadata(ctx, &anime)

	response.WriteJSON(w, http.StatusOK, anime)
}

func (h *VideoHandlers) FetchAnimeMetadata(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 60*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	anime, err := h.repo.GetAnimeByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	if anime.CoverImageURL != nil && *anime.CoverImageURL != "" {
		response.WriteJSON(w, http.StatusOK, map[string]interface{}{
			"cover_image_url": anime.CoverImageURL,
			"message":         "Cover image already exists",
		})
		return
	}

	var coverImageURL string

	if h.aniRepo != nil {
		candidates, err := h.aniRepo.SearchMedia(ctx, anime.Title, 10)
		if err == nil && len(candidates) > 0 {
			normalizedTitle := strings.ToLower(strings.TrimSpace(anime.Title))
			bestMatch := ""
			bestScore := 0.0

			for _, candidate := range candidates {
				media, err := h.aniRepo.GetByID(ctx, candidate.ID)
				if err == nil && media.CoverImage != "" {
					candidateTitle := ""
					if candidate.TitleRomaji.Valid {
						candidateTitle = strings.ToLower(strings.TrimSpace(candidate.TitleRomaji.String))
					}
					if candidate.TitleEnglish.Valid && candidateTitle == "" {
						candidateTitle = strings.ToLower(strings.TrimSpace(candidate.TitleEnglish.String))
					}
					if candidateTitle == "" && candidate.TitleNative.Valid {
						candidateTitle = strings.ToLower(strings.TrimSpace(candidate.TitleNative.String))
					}

					score := 0.0
					if candidateTitle == normalizedTitle {
						score = 1.0
					} else if candidateTitle != "" && normalizedTitle != "" {
						if strings.Contains(normalizedTitle, candidateTitle) || strings.Contains(candidateTitle, normalizedTitle) {
							score = 0.8
						} else {
							words := strings.Fields(normalizedTitle)
							matched := 0
							for _, word := range words {
								if len(word) > 2 && strings.Contains(candidateTitle, word) {
									matched++
								}
							}
							if len(words) > 0 {
								score = float64(matched) / float64(len(words))
							}
						}
					}

					if score > bestScore {
						bestScore = score
						bestMatch = media.CoverImage
					}

					if score >= 1.0 {
						coverImageURL = media.CoverImage
						break
					}
				}
			}

			if coverImageURL == "" && bestMatch != "" && bestScore >= 0.7 {
				coverImageURL = bestMatch
			}
		}
	}

	if coverImageURL == "" {
		episodes, _, err := h.repo.ListEpisodesByAnime(ctx, id, 1, 1000)
		if err == nil && len(episodes) > 0 {
			firstEpisode := episodes[0]
			for _, ep := range episodes {
				if ep.EpisodeNumber != nil && *ep.EpisodeNumber == 1 {
					firstEpisode = ep
					break
				}
			}

			if video.IsFFMpegAvailable() {
				coverDir := filepath.Join(filepath.Dir(firstEpisode.FilePath), ".covers")
				if err := os.MkdirAll(coverDir, 0755); err == nil {
					coverPath := filepath.Join(coverDir, "cover.jpg")
					if _, err := os.Stat(coverPath); os.IsNotExist(err) {
						timestamp := 60.0
						if firstEpisode.Duration != nil && *firstEpisode.Duration > 120 {
							timestamp = *firstEpisode.Duration * 0.1
						}
										if err := video.ExtractCoverImage(firstEpisode.FilePath, coverPath, timestamp); err == nil {
							if absPath, err := filepath.Abs(coverPath); err == nil {
								coverImageURL = absPath
							} else {
								coverImageURL = coverPath
							}
						}
					} else {
						if absPath, err := filepath.Abs(coverPath); err == nil {
							coverImageURL = absPath
						} else {
							coverImageURL = coverPath
						}
					}
				}
			}
		}
	}

	if coverImageURL != "" {
		if err := h.repo.UpdateAnimeCoverImage(ctx, id, coverImageURL); err != nil {
			response.WriteError(w, http.StatusInternalServerError, err)
			return
		}
		response.WriteJSON(w, http.StatusOK, map[string]interface{}{
			"cover_image_url": coverImageURL,
		})
	} else {
		response.WriteJSON(w, http.StatusNotFound, map[string]string{
			"error": "No cover image found for this anime",
		})
	}
}

func (h *VideoHandlers) EpisodesList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	animeID, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	page, pageSize := response.ParsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 500)

	episodes, total, err := h.repo.ListEpisodesByAnime(ctx, animeID, page, pageSize)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, response.ListResponse[model.Episode]{
		Data: episodes,
		Pagination: response.PaginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
			HasMore:  page*pageSize < total,
		},
	})
}

func (h *VideoHandlers) EpisodeGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	episode, err := h.repo.GetEpisodeByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, episode)
}

func (h *VideoHandlers) ThumbnailsList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	episodeID, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	thumbnails, err := h.repo.ListThumbnailsByEpisode(ctx, episodeID)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, thumbnails)
}

func (h *VideoHandlers) ServeImage(w http.ResponseWriter, r *http.Request) {
	path := r.URL.Query().Get("path")
	if path == "" {
		response.WriteError(w, http.StatusBadRequest, fmt.Errorf("path parameter is required"))
		return
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		response.WriteError(w, http.StatusNotFound, fmt.Errorf("image not found"))
		return
	}

	ext := filepath.Ext(path)
	contentType := "image/jpeg"
	if ext == ".png" {
		contentType = "image/png"
	} else if ext == ".webp" {
		contentType = "image/webp"
	}

	file, err := os.Open(path)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}
	defer file.Close()

	w.Header().Set("Content-Type", contentType)
	http.ServeContent(w, r, filepath.Base(path), time.Time{}, file)
}

func (h *VideoHandlers) ClearCoverCache(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()

	if err := h.repo.ClearAllCoverImages(ctx); err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, map[string]string{
		"message": "Cover cache cleared successfully",
	})
}

func (h *VideoHandlers) Search(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	query := r.URL.Query().Get("q")
	if query == "" {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "query parameter 'q' is required",
		})
		return
	}

	limitStr := r.URL.Query().Get("limit")
	limit := 10
	if l, err := strconv.Atoi(limitStr); err == nil && l > 0 && l <= 50 {
		limit = l
	}

	results, err := h.searchService.Search(ctx, query, limit)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	type searchResult struct {
		ID      int     `json:"id"`
		Title   string  `json:"title"`
		Score   float64 `json:"score"`
		Matches []string `json:"matches,omitempty"`
	}

	searchResults := make([]searchResult, 0, len(results))
	for _, res := range results {
		searchResults = append(searchResults, searchResult{
			ID:      res.Anime.ID,
			Title:   res.Anime.Title,
			Score:   res.Score,
			Matches: res.Matches,
		})
	}

	response.WriteJSON(w, http.StatusOK, searchResults)
}

func (h *VideoHandlers) TriggerScan(w http.ResponseWriter, r *http.Request) {
	scanPath := r.URL.Query().Get("path")
	if scanPath == "" {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "path parameter is required",
		})
		return
	}

	h.scanMu.Lock()
	if h.scanning {
		h.scanMu.Unlock()
		response.WriteJSON(w, http.StatusConflict, map[string]string{
			"error": "scan already in progress",
		})
		return
	}
	h.scanning = true
	h.scanMu.Unlock()

	go func() {
		defer func() {
			h.scanMu.Lock()
			h.scanning = false
			h.scanMu.Unlock()
		}()

		scanCtx, scanCancel := context.WithTimeout(context.Background(), 30*time.Minute)
		defer scanCancel()

		_ = h.indexer.IndexPath(scanCtx, scanPath)
	}()

	response.WriteJSON(w, http.StatusAccepted, map[string]string{
		"status": "scan started",
		"path":   scanPath,
	})
}

func (h *VideoHandlers) ScanStatus(w http.ResponseWriter, r *http.Request) {
	h.scanMu.Lock()
	scanning := h.scanning
	h.scanMu.Unlock()

	status := "idle"
	if scanning {
		status = "scanning"
	}

	response.WriteJSON(w, http.StatusOK, map[string]interface{}{
		"status": status,
	})
}

func (h *VideoHandlers) UpdateEpisodeNumbers(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Minute)
	defer cancel()

	updated, err := indexer.UpdateEpisodeNumbers(ctx, h.repo)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, map[string]interface{}{
		"message": "episode numbers updated successfully",
		"updated": updated,
	})
}

func (h *VideoHandlers) ExtractThumbnails(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Minute)
	defer cancel()

	extracted, err := indexer.ExtractThumbnailsForEpisodes(ctx, h.repo)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, map[string]interface{}{
		"message":   "thumbnails extracted successfully",
		"extracted": extracted,
	})
}

func (h *VideoHandlers) AnimeListByLibrary(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	libraryID, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	library, err := h.repo.GetLibraryByID(ctx, libraryID)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	page, pageSize := response.ParsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 500)
	search := r.URL.Query().Get("search")

	animeList, total, err := h.repo.ListAnimeByLibrary(ctx, library.Path, search, page, pageSize)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	for i := range animeList {
		h.ensureAniListMetadata(ctx, &animeList[i])
	}

	response.WriteJSON(w, http.StatusOK, response.ListResponse[model.Anime]{
		Data: animeList,
		Pagination: response.PaginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
			HasMore:  page*pageSize < total,
		},
	})
}

func (h *VideoHandlers) ListLibraries(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	libraries, err := h.repo.ListLibraries(ctx)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, libraries)
}

func (h *VideoHandlers) CreateLibrary(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	var req struct {
		Path string  `json:"path"`
		Name *string `json:"name,omitempty"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "invalid request body",
		})
		return
	}

	if req.Path == "" {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "path is required",
		})
		return
	}

	id, err := h.repo.CreateLibrary(ctx, req.Path, req.Name)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	library, err := h.repo.GetLibraryByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	libraries, err := h.repo.ListLibraries(ctx)
	if err == nil && len(libraries) == 1 {
		go func() {
			cleanupCtx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			h.repo.CleanupOrphanedAnime(cleanupCtx)
		}()
	}

	response.WriteJSON(w, http.StatusCreated, library)
}

func (h *VideoHandlers) GetLibrary(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	library, err := h.repo.GetLibraryByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, library)
}

func (h *VideoHandlers) UpdateLibrary(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	var req struct {
		Name *string `json:"name,omitempty"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "invalid request body",
		})
		return
	}

	if err := h.repo.UpdateLibrary(ctx, id, req.Name); err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	library, err := h.repo.GetLibraryByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, library)
}

func (h *VideoHandlers) DeleteLibrary(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	if err := h.repo.DeleteLibrary(ctx, id); err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusNoContent, nil)
}

func (h *VideoHandlers) GetSettings(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	settings, err := h.repo.GetAllSettings(ctx)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, settings)
}

func (h *VideoHandlers) UpdateSettings(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	var req map[string]string
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "invalid request body",
		})
		return
	}

	for key, value := range req {
		if err := h.repo.SetSetting(ctx, key, value); err != nil {
			response.WriteError(w, http.StatusInternalServerError, err)
			return
		}
	}

	settings, err := h.repo.GetAllSettings(ctx)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, settings)
}

func (h *VideoHandlers) GetSetting(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	key := chi.URLParam(r, "key")
	if key == "" {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "key parameter is required",
		})
		return
	}

	setting, err := h.repo.GetSetting(ctx, key)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}
	if setting == nil {
		response.WriteJSON(w, http.StatusNotFound, map[string]string{
			"error": "setting not found",
		})
		return
	}

	response.WriteJSON(w, http.StatusOK, setting)
}

func (h *VideoHandlers) DeleteSetting(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	key := chi.URLParam(r, "key")
	if key == "" {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "key parameter is required",
		})
		return
	}

	if err := h.repo.DeleteSetting(ctx, key); err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusNoContent, nil)
}

func (h *VideoHandlers) StreamEpisode(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Minute)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	episode, err := h.repo.GetEpisodeByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	if _, err := os.Stat(episode.FilePath); err != nil {
		if os.IsNotExist(err) {
			response.WriteJSON(w, http.StatusNotFound, map[string]string{
				"error": "video file not found",
			})
			return
		}
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	filePath := episode.FilePath
	
	transcode := r.URL.Query().Get("transcode")
	if transcode == "true" || transcode == "1" {
		config := h.getTranscodeConfig(ctx)
		
		transcodedPath, err := h.transcodeService.GetTranscodedPath(ctx, episode.FilePath, config)
		if err != nil {
			response.WriteError(w, http.StatusInternalServerError, fmt.Errorf("transcode failed: %w", err))
			return
		}
		filePath = transcodedPath
	}

	if err := video.ServeVideoFile(w, r, filePath); err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}
}

func (h *VideoHandlers) getTranscodeConfig(ctx context.Context) service.TranscodeConfig {
	settings, err := h.repo.GetAllSettings(ctx)
	if err != nil {
		return service.TranscodeConfig{
			EnableTranscoding: false,
			Preset:           "fast",
			Container:        "mp4",
		}
	}
	
	getSetting := func(key, defaultValue string) string {
		if val, ok := settings[key]; ok && val != "" {
			return val
		}
		return defaultValue
	}
	
	getBoolSetting := func(key string, defaultValue bool) bool {
		if val, ok := settings[key]; ok {
			return val == "true" || val == "1"
		}
		return defaultValue
	}
	
	hardwareEncoder := getSetting("transcode_hardware_encoder", "auto")
	hardwareAccel := getSetting("transcode_hardware_acceleration", "")
	preset := getSetting("transcode_preset", "fast")
	resolution := getSetting("transcode_resolution", "")
	videoCodec := getSetting("transcode_video_codec", "")
	audioCodec := getSetting("transcode_audio_codec", "aac")
	tune := getSetting("transcode_tune", "")
	container := getSetting("transcode_container", "mp4")
	remuxOnly := getBoolSetting("transcode_remux_only", false)
	enableTranscoding := getBoolSetting("transcode_enabled", false)
	
	return service.TranscodeConfig{
		EnableTranscoding:    enableTranscoding,
		HardwareEncoder:      hardwareEncoder,
		HardwareAcceleration: hardwareAccel,
		Preset:               preset,
		Resolution:           resolution,
		VideoCodec:           videoCodec,
		AudioCodec:           audioCodec,
		Tune:                 tune,
		Container:            container,
		RemuxOnly:            remuxOnly,
	}
}

func (h *VideoHandlers) GetHardwareInfo(w http.ResponseWriter, r *http.Request) {
	detection := video.DetectHardware()
	
	info := map[string]interface{}{
		"has_hardware":     detection.HasHardware(),
		"default_encoder":  video.GetDefaultHardwareEncoder(),
		"default_accel":    video.GetDefaultHardwareAcceleration(),
		"available_encoders": map[string]bool{},
	}
	
	if detection.NVIDIA != nil {
		info["nvidia"] = map[string]interface{}{
			"name":        detection.NVIDIA.Name,
			"encoder":     detection.NVIDIA.Encoder,
			"decoder":     detection.NVIDIA.Decoder,
			"acceleration": detection.NVIDIA.Acceleration,
			"available":   detection.NVIDIA.Available,
		}
		info["available_encoders"].(map[string]bool)["h264_nvenc"] = true
		info["available_encoders"].(map[string]bool)["hevc_nvenc"] = true
	}
	
	if detection.Intel != nil {
		info["intel"] = map[string]interface{}{
			"name":         detection.Intel.Name,
			"encoder":      detection.Intel.Encoder,
			"decoder":      detection.Intel.Decoder,
			"acceleration": detection.Intel.Acceleration,
			"available":    detection.Intel.Available,
		}
		info["available_encoders"].(map[string]bool)["h264_qsv"] = true
		info["available_encoders"].(map[string]bool)["hevc_qsv"] = true
	}
	
	if detection.AMD != nil {
		info["amd"] = map[string]interface{}{
			"name":         detection.AMD.Name,
			"encoder":      detection.AMD.Encoder,
			"decoder":      detection.AMD.Decoder,
			"acceleration": detection.AMD.Acceleration,
			"available":    detection.AMD.Available,
		}
		info["available_encoders"].(map[string]bool)["h264_amf"] = true
		info["available_encoders"].(map[string]bool)["hevc_amf"] = true
	}
	
	best := detection.GetBestEncoder()
	if best != nil {
		info["best_encoder"] = map[string]interface{}{
			"name":         best.Name,
			"encoder":      best.Encoder,
			"decoder":      best.Decoder,
			"acceleration": best.Acceleration,
		}
	}
	
	response.WriteJSON(w, http.StatusOK, info)
}

func (h *VideoHandlers) ensureAniListMetadata(ctx context.Context, anime *model.Anime) {
	if h.aniRepo == nil {
		return
	}

	if anime.Title == "" {
		return
	}

	rawTitle := filepath.Base(anime.FolderPath)
	bracketPattern := regexp.MustCompile(`\[([^\]]+)\]`)
	rawTitle = bracketPattern.ReplaceAllString(rawTitle, "")
	
	episodeRangePattern := regexp.MustCompile(`(?i)\s*-\s*\d{2,}\s*~\s*\d{2,}\s*`)
	rawTitle = episodeRangePattern.ReplaceAllString(rawTitle, " ")
	
	hashPattern := regexp.MustCompile(`(?i)\s+[A-F0-9]{8,}\s*`)
	rawTitle = hashPattern.ReplaceAllString(rawTitle, " ")
	
	rawTitle = strings.TrimSpace(rawTitle)
	
	multiSpacePattern := regexp.MustCompile(`\s+`)
	rawTitle = multiSpacePattern.ReplaceAllString(rawTitle, " ")
	
	rawTitle = strings.TrimSpace(rawTitle)
	
	if rawTitle == "" {
		rawTitle = anime.Title
	}

	animeSeason, hasAnimeSeason := util.ExtractSeasonNumber(rawTitle)
	animePart, hasAnimePart := util.ExtractPartNumber(rawTitle)
	
	needsRecheck := false
	if anime.AniListID == nil {
		needsRecheck = true
	} else if anime.CoverImageURL == nil || *anime.CoverImageURL == "" {
		needsRecheck = true
	} else if anime.AniListMetadata == nil {
		needsRecheck = true
	} else if hasAnimeSeason {
		mediaTitle := strings.ToLower(anime.AniListMetadata.Title.Romaji + " " + anime.AniListMetadata.Title.English)
		mediaSeason, hasMediaSeason := util.ExtractSeasonNumber(mediaTitle)
		if !hasMediaSeason || mediaSeason != animeSeason {
			needsRecheck = true
		}
		if hasAnimePart {
			mediaPart, hasMediaPart := util.ExtractPartNumber(mediaTitle)
			if !hasMediaPart || mediaPart != animePart {
				needsRecheck = true
			}
		}
	}

	if !needsRecheck {
		if anime.AniListID != nil && (anime.CoverImageURL == nil || *anime.CoverImageURL == "" || anime.AniListMetadata == nil) {
			media, err := h.aniRepo.GetByID(ctx, *anime.AniListID)
			if err == nil {
				if anime.CoverImageURL == nil || *anime.CoverImageURL == "" {
					h.repo.UpdateAnimeCoverImage(ctx, anime.ID, media.CoverImage)
					anime.CoverImageURL = &media.CoverImage
				}
				if anime.AniListMetadata == nil {
					h.repo.UpdateAnimeAniList(ctx, anime.ID, *anime.AniListID, &media)
					anime.AniListMetadata = &media
				}
			}
		}
		return
	}

	format := h.detectFormat(anime.FolderPath)
	if format == nil {
		format = h.detectFormat(rawTitle)
	}
	if format == nil {
		episodes, _, err := h.repo.ListEpisodesByAnime(ctx, anime.ID, 1, 1000)
		if err == nil {
			for _, episode := range episodes {
				if detectedFormat := h.detectFormat(episode.Filename); detectedFormat != nil {
					format = detectedFormat
					break
				}
			}
		}
	}
	results, _, err := service.HandleImprovedAniListSearch(ctx, h.aniRepo, rawTitle, format, 3)
	if err != nil || len(results) == 0 {
		return
	}

	if len(results) == 0 {
		return
	}

	bestMatch := results[0].ID
	bestScore := results[0].Score
	const minScoreThreshold = 0.05
	
	if bestScore < minScoreThreshold {
		return
	}
	
	rawTitleLower := strings.ToLower(rawTitle)
	
	if anime.AniListID == nil || *anime.AniListID != bestMatch {
		betterMatch := results[0].ID
		betterScore := results[0].Score
		
		if hasAnimeSeason && results[0].SeasonNumber != 0 && results[0].SeasonNumber != animeSeason {
			if len(results) > 1 {
				for i := 1; i < len(results); i++ {
					if results[i].Score >= minScoreThreshold && results[i].SeasonNumber == animeSeason {
						if !hasAnimePart || results[i].PartNumber == animePart || results[i].PartNumber == 0 {
							betterMatch = results[i].ID
							betterScore = results[i].Score
							break
						}
					}
				}
			}
			if betterMatch == results[0].ID && results[0].SeasonNumber != animeSeason {
				return
			}
		}
		
		if hasAnimePart && results[0].PartNumber != 0 && results[0].PartNumber != animePart {
			if len(results) > 1 {
				for i := 1; i < len(results); i++ {
					if results[i].Score >= minScoreThreshold && results[i].PartNumber == animePart {
						if !hasAnimeSeason || results[i].SeasonNumber == animeSeason || results[i].SeasonNumber == 0 {
							betterMatch = results[i].ID
							betterScore = results[i].Score
							break
						}
					}
				}
			}
			if betterMatch == results[0].ID && results[0].PartNumber != animePart {
				parts := strings.Split(rawTitleLower, " - ")
				if len(parts) > 1 {
					lastPart := strings.TrimSpace(parts[len(parts)-1])
					if lastPart != "" && !strings.HasPrefix(lastPart, strings.ToLower("01")) {
						for i := 1; i < len(results); i++ {
							if results[i].Score >= minScoreThreshold {
								tempMedia, _ := h.aniRepo.GetByID(ctx, results[i].ID)
								if tempMedia.Title.Romaji != "" || tempMedia.Title.English != "" {
									tempTitle := strings.ToLower(tempMedia.Title.Romaji + " " + tempMedia.Title.English + " " + tempMedia.Title.Native)
									if strings.Contains(tempTitle, lastPart) {
										betterMatch = results[i].ID
										betterScore = results[i].Score
										break
									}
								}
							}
						}
					}
				}
				if betterMatch == results[0].ID {
					return
				}
			}
		}
		
		if betterMatch != results[0].ID {
			bestMatch = betterMatch
			bestScore = betterScore
		}
		
		media, err := h.aniRepo.GetByID(ctx, bestMatch)
		if err == nil {
			h.repo.UpdateAnimeAniList(ctx, anime.ID, bestMatch, &media)
			if media.CoverImage != "" {
				h.repo.UpdateAnimeCoverImage(ctx, anime.ID, media.CoverImage)
			}
			anime.AniListID = &bestMatch
			anime.AniListMetadata = &media
			if media.CoverImage != "" {
				anime.CoverImageURL = &media.CoverImage
			}
		}
	} else if anime.CoverImageURL == nil || *anime.CoverImageURL == "" || anime.AniListMetadata == nil {
		media, err := h.aniRepo.GetByID(ctx, bestMatch)
		if err == nil {
			if anime.CoverImageURL == nil || *anime.CoverImageURL == "" {
				h.repo.UpdateAnimeCoverImage(ctx, anime.ID, media.CoverImage)
				anime.CoverImageURL = &media.CoverImage
			}
			if anime.AniListMetadata == nil {
				h.repo.UpdateAnimeAniList(ctx, anime.ID, bestMatch, &media)
				anime.AniListMetadata = &media
			}
		}
	}
}

func (h *VideoHandlers) cleanAnimeTitle(title string) string {
	bracketPattern := regexp.MustCompile(`\[([^\]]+)\]`)
	title = bracketPattern.ReplaceAllString(title, "")
	
	episodeRangePattern := regexp.MustCompile(`(?i)\s*-\s*\d{2,}\s*~\s*\d{2,}\s*`)
	title = episodeRangePattern.ReplaceAllString(title, "")
	
	fileExtensionPattern := regexp.MustCompile(`(?i)\.(mkv|mp4|avi|webm|mov|flv|wmv|m4v)$`)
	title = fileExtensionPattern.ReplaceAllString(title, "")
	
	resolutionPattern := regexp.MustCompile(`(?i)\s+\d{3,4}p\s*`)
	title = resolutionPattern.ReplaceAllString(title, " ")
	
	codecPattern := regexp.MustCompile(`(?i)\s+(HEVC|H264|AVC|X264|X265|VP9|AV1|AAC|AC3|DTS|FLAC)\s*`)
	title = codecPattern.ReplaceAllString(title, " ")
	
	webPattern := regexp.MustCompile(`(?i)\s+(WEBRip|BDRip|BluRay|DVD|WEB|CR|TV|RAW)\s*`)
	title = webPattern.ReplaceAllString(title, " ")
	
	hashPattern := regexp.MustCompile(`(?i)\s+[A-F0-9]{8,}\s*`)
	title = hashPattern.ReplaceAllString(title, " ")
	
	title = strings.TrimSpace(title)
	
	multiSpacePattern := regexp.MustCompile(`\s+`)
	title = multiSpacePattern.ReplaceAllString(title, " ")
	
	title = strings.TrimSpace(title)
	return title
}

func (h *VideoHandlers) detectFormat(titleOrPath string) *string {
	titleLower := strings.ToLower(titleOrPath)
	
	if strings.Contains(titleLower, "movie") {
		format := "MOVIE"
		return &format
	}
	if strings.Contains(titleLower, "ova") {
		format := "OVA"
		return &format
	}
	if strings.Contains(titleLower, "special") {
		format := "SPECIAL"
		return &format
	}
	if strings.Contains(titleLower, "ona") {
		format := "ONA"
		return &format
	}
	
	return nil
}

