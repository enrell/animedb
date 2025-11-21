package cache

import (
	"testing"
	"time"
)

func TestLRUCache_BasicOperations(t *testing.T) {
	cache := NewLRUCache(100, 5*time.Minute)

	cache.Set("key1", "value1")
	cache.Set("key2", "value2")

	if val, ok := cache.Get("key1"); !ok || val != "value1" {
		t.Errorf("expected value1, got %v", val)
	}

	if val, ok := cache.Get("key2"); !ok || val != "value2" {
		t.Errorf("expected value2, got %v", val)
	}

	if _, ok := cache.Get("key3"); ok {
		t.Error("expected key3 to not exist")
	}
}

func TestLRUCache_Expiration(t *testing.T) {
	cache := NewLRUCache(100, 100*time.Millisecond)

	cache.Set("key1", "value1")

	time.Sleep(50 * time.Millisecond)
	if _, ok := cache.Get("key1"); !ok {
		t.Error("expected key1 to still be valid")
	}

	time.Sleep(60 * time.Millisecond)
	if _, ok := cache.Get("key1"); ok {
		t.Error("expected key1 to be expired")
	}
}

func TestLRUCache_Eviction(t *testing.T) {
	cache := NewLRUCache(3, 5*time.Minute)

	cache.Set("key1", "value1")
	cache.Set("key2", "value2")
	cache.Set("key3", "value3")
	cache.Set("key4", "value4")

	if _, ok := cache.Get("key1"); ok {
		t.Error("expected key1 to be evicted")
	}

	if _, ok := cache.Get("key4"); !ok {
		t.Error("expected key4 to exist")
	}
}

func TestLRUCache_Update(t *testing.T) {
	cache := NewLRUCache(100, 5*time.Minute)

	cache.Set("key1", "value1")
	cache.Set("key1", "value2")

	if val, ok := cache.Get("key1"); !ok || val != "value2" {
		t.Errorf("expected value2, got %v", val)
	}
}

func TestLRUCache_Remove(t *testing.T) {
	cache := NewLRUCache(100, 5*time.Minute)

	cache.Set("key1", "value1")
	cache.Remove("key1")

	if _, ok := cache.Get("key1"); ok {
		t.Error("expected key1 to be removed")
	}
}

func TestLRUCache_Clear(t *testing.T) {
	cache := NewLRUCache(100, 5*time.Minute)

	cache.Set("key1", "value1")
	cache.Set("key2", "value2")
	cache.Clear()

	if cache.Size() != 0 {
		t.Errorf("expected size 0, got %d", cache.Size())
	}
}

func TestLRUCache_Size(t *testing.T) {
	cache := NewLRUCache(100, 5*time.Minute)

	if cache.Size() != 0 {
		t.Errorf("expected size 0, got %d", cache.Size())
	}

	cache.Set("key1", "value1")
	if cache.Size() != 1 {
		t.Errorf("expected size 1, got %d", cache.Size())
	}

	cache.Set("key2", "value2")
	if cache.Size() != 2 {
		t.Errorf("expected size 2, got %d", cache.Size())
	}
}

func TestLRUCache_Cleanup(t *testing.T) {
	cache := NewLRUCache(100, 50*time.Millisecond)

	cache.Set("key1", "value1")
	cache.Set("key2", "value2")

	time.Sleep(60 * time.Millisecond)
	cache.Cleanup()

	if cache.Size() != 0 {
		t.Errorf("expected size 0 after cleanup, got %d", cache.Size())
	}
}

