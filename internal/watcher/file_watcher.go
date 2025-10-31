package watcher

import (
	"context"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
)

type Watcher interface {
	Watch(ctx context.Context, paths []string, handler EventHandler) error
	Stop() error
}

type EventHandler func(ctx context.Context, event Event) error

type Event struct {
	Path      string
	Op        Operation
	IsDir     bool
	Timestamp time.Time
}

type Operation int

const (
	Create Operation = iota
	Write
	Remove
	Rename
)

type fileWatcher struct {
	watcher   *fsnotify.Watcher
	debounce  time.Duration
	mu        sync.Mutex
	stopFunc  func()
	debouncer map[string]*debounceTimer
}

type WatcherOptions struct {
	Debounce time.Duration
}

func NewWatcher(opts WatcherOptions) (Watcher, error) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	if opts.Debounce == 0 {
		opts.Debounce = 500 * time.Millisecond
	}

	return &fileWatcher{
		watcher:   watcher,
		debounce:  opts.Debounce,
		debouncer: make(map[string]*debounceTimer),
	}, nil
}

func (w *fileWatcher) Watch(ctx context.Context, paths []string, handler EventHandler) error {
	ctx, cancel := context.WithCancel(ctx)
	w.mu.Lock()
	w.stopFunc = cancel
	w.mu.Unlock()

	for _, path := range paths {
		if err := w.watcher.Add(path); err != nil {
			cancel()
			return err
		}
		if err := w.watchRecursive(path); err != nil {
			cancel()
			return err
		}
	}

	go w.processEvents(ctx, handler)

	<-ctx.Done()
	return w.watcher.Close()
}

func (w *fileWatcher) watchRecursive(root string) error {
	return filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if info.IsDir() {
			return w.watcher.Add(path)
		}
		return nil
	})
}

func (w *fileWatcher) processEvents(ctx context.Context, handler EventHandler) {
	for {
		select {
		case <-ctx.Done():
			return
		case event, ok := <-w.watcher.Events:
			if !ok {
				return
			}
			w.handleEvent(ctx, event, handler)
		case err, ok := <-w.watcher.Errors:
			if !ok {
				return
			}
			if err != nil {
				handler(ctx, Event{
					Path:      "",
					Op:        Remove,
					IsDir:     false,
					Timestamp: time.Now(),
				})
			}
		}
	}
}

func (w *fileWatcher) handleEvent(ctx context.Context, fsEvent fsnotify.Event, handler EventHandler) {
	op := w.mapOperation(fsEvent.Op)

	if op == Create {
		info, err := filepath.Abs(fsEvent.Name)
		if err == nil {
			if stat, err := os.Stat(info); err == nil && stat.IsDir() {
				w.watcher.Add(info)
			}
		}
	}

	w.mu.Lock()
	timer, exists := w.debouncer[fsEvent.Name]
	if !exists {
		timer = newDebounceTimer(w.debounce, func() {
			w.mu.Lock()
			delete(w.debouncer, fsEvent.Name)
			w.mu.Unlock()

			info, _ := os.Stat(fsEvent.Name)
			isDir := info != nil && info.IsDir()

			handler(ctx, Event{
				Path:      fsEvent.Name,
				Op:        op,
				IsDir:     isDir,
				Timestamp: time.Now(),
			})
		})
		w.debouncer[fsEvent.Name] = timer
	}
	timer.reset()
	w.mu.Unlock()
}

func (w *fileWatcher) mapOperation(op fsnotify.Op) Operation {
	if op&fsnotify.Create == fsnotify.Create {
		return Create
	}
	if op&fsnotify.Write == fsnotify.Write {
		return Write
	}
	if op&fsnotify.Remove == fsnotify.Remove {
		return Remove
	}
	if op&fsnotify.Rename == fsnotify.Rename {
		return Rename
	}
	return Write
}

func (w *fileWatcher) Stop() error {
	w.mu.Lock()
	if w.stopFunc != nil {
		w.stopFunc()
	}
	w.mu.Unlock()
	return w.watcher.Close()
}

type debounceTimer struct {
	timer    *time.Timer
	duration time.Duration
	callback func()
	mu       sync.Mutex
}

func newDebounceTimer(duration time.Duration, callback func()) *debounceTimer {
	return &debounceTimer{
		duration: duration,
		callback: callback,
		timer:    time.NewTimer(duration),
	}
}

func (dt *debounceTimer) reset() {
	dt.mu.Lock()
	defer dt.mu.Unlock()

	if !dt.timer.Stop() {
		<-dt.timer.C
	}

	dt.timer.Reset(dt.duration)
	go func() {
		<-dt.timer.C
		dt.callback()
	}()
}

