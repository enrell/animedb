package main

import (
	"context"
	"flag"
	"log"
	"os"
	"os/signal"
	"sync"
	"syscall"

	"animedb/internal/scanner"
)

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var rootPath string
	var workerCount int

	flag.StringVar(&rootPath, "path", "", "Root directory to scan for video files")
	flag.IntVar(&workerCount, "workers", 15, "Number of worker goroutines")
	flag.Parse()

	if rootPath == "" {
		log.Fatal("--path is required")
	}

	sc := scanner.NewScanner(scanner.ScanOptions{
		WorkerCount: workerCount,
	})

	log.Printf("Starting scan of %s with %d workers", rootPath, workerCount)

	count := 0
	var mu sync.Mutex

	err := sc.Scan(ctx, rootPath, func(ctx context.Context, filePath string) error {
		mu.Lock()
		count++
		currentCount := count
		mu.Unlock()

		if currentCount%100 == 0 {
			log.Printf("Processed %d files...", currentCount)
		}

		return nil
	})

	if err != nil {
		log.Fatalf("Scan error: %v", err)
	}

	log.Printf("Scan complete. Total files processed: %d", count)
}

