package video

import (
	"crypto/sha256"
	"fmt"
	"io"
	"os"
)

func ComputeHash(filePath string) (string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return "", fmt.Errorf("open file: %w", err)
	}
	defer file.Close()

	hasher := sha256.New()

	buf := make([]byte, 64*1024)
	for {
		n, err := file.Read(buf)
		if n > 0 {
			if _, err := hasher.Write(buf[:n]); err != nil {
				return "", fmt.Errorf("write to hasher: %w", err)
			}
		}
		if err == io.EOF {
			break
		}
		if err != nil {
			return "", fmt.Errorf("read file: %w", err)
		}
	}

	hash := fmt.Sprintf("%x", hasher.Sum(nil))
	return hash, nil
}

func VerifyHash(filePath, expectedHash string) (bool, error) {
	actualHash, err := ComputeHash(filePath)
	if err != nil {
		return false, err
	}
	return actualHash == expectedHash, nil
}

