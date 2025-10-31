package scanner

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

var videoExtensions = map[string]bool{
	".mp4":  true,
	".mkv":  true,
	".avi":  true,
	".webm": true,
	".mov":  true,
}

type Scanner interface {
	Scan(ctx context.Context, rootPath string, handler FileHandler) error
}

type FileHandler func(ctx context.Context, filePath string) error

type videoScanner struct {
	workerCount int
	skipFiles   map[string]bool
	mu          sync.RWMutex
}

type ScanOptions struct {
	WorkerCount int
	SkipFiles   []string
}

func NewScanner(opts ScanOptions) Scanner {
	skipMap := make(map[string]bool)
	for _, f := range opts.SkipFiles {
		skipMap[f] = true
	}
	if opts.WorkerCount <= 0 {
		opts.WorkerCount = 15
	}
	return &videoScanner{
		workerCount: opts.WorkerCount,
		skipFiles:   skipMap,
	}
}

func (s *videoScanner) Scan(ctx context.Context, rootPath string, handler FileHandler) error {
	if _, err := os.Stat(rootPath); err != nil {
		return fmt.Errorf("root path does not exist: %w", err)
	}

	type job struct {
		path string
	}

	fileQueue := make(chan job, 1000)
	var wg sync.WaitGroup
	var handlerErr error
	var handlerMu sync.Mutex

	for i := 0; i < s.workerCount; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				select {
				case <-ctx.Done():
					return
				case j, ok := <-fileQueue:
					if !ok {
						return
					}
					if err := handler(ctx, j.path); err != nil {
						handlerMu.Lock()
						if handlerErr == nil {
							handlerErr = err
						}
						handlerMu.Unlock()
					}
				}
			}
		}()
	}

	scanErr := filepath.WalkDir(rootPath, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		if d.IsDir() {
			return nil
		}

		ext := strings.ToLower(filepath.Ext(path))
		if !videoExtensions[ext] {
			return nil
		}

		s.mu.RLock()
		skip := s.skipFiles[path]
		s.mu.RUnlock()
		if skip {
			return nil
		}

		select {
		case fileQueue <- job{path: path}:
		case <-ctx.Done():
			return ctx.Err()
		}

		return nil
	})

	close(fileQueue)
	wg.Wait()

	if scanErr != nil {
		return scanErr
	}

	handlerMu.Lock()
	defer handlerMu.Unlock()
	return handlerErr
}

func IsVideoFile(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))
	return videoExtensions[ext]
}

