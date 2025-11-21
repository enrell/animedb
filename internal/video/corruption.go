package video

import (
	"fmt"
	"os"
)

func CheckPartialFile(filePath string, expectedSize int64) (bool, error) {
	info, err := os.Stat(filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, fmt.Errorf("stat file: %w", err)
	}

	if info.Size() < expectedSize {
		return true, nil
	}

	return false, nil
}

func CheckFileCorruption(filePath, expectedHash string) (bool, error) {
	if expectedHash == "" {
		return false, nil
	}

	isValid, err := VerifyHash(filePath, expectedHash)
	if err != nil {
		return false, err
	}

	return !isValid, nil
}

func DetectFileIssues(filePath string, expectedSize int64, expectedHash string) (isPartial bool, isCorrupted bool, err error) {
	isPartial, err = CheckPartialFile(filePath, expectedSize)
	if err != nil {
		return false, false, err
	}

	if expectedHash != "" {
		isCorrupted, err = CheckFileCorruption(filePath, expectedHash)
		if err != nil {
			return false, false, err
		}
	}

	return isPartial, isCorrupted, nil
}

