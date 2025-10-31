package cache

import (
	"container/list"
	"sync"
	"time"
)

type entry struct {
	key        string
	value      interface{}
	expiresAt  time.Time
	element    *list.Element
}

type LRUCache struct {
	mu       sync.RWMutex
	ttl      time.Duration
	maxSize  int
	items    map[string]*entry
	evictList *list.List
}

func NewLRUCache(maxSize int, ttl time.Duration) *LRUCache {
	return &LRUCache{
		ttl:       ttl,
		maxSize:  maxSize,
		items:    make(map[string]*entry),
		evictList: list.New(),
	}
}

func (c *LRUCache) Get(key string) (interface{}, bool) {
	c.mu.RLock()
	item, exists := c.items[key]
	c.mu.RUnlock()

	if !exists {
		return nil, false
	}

	c.mu.Lock()
	defer c.mu.Unlock()

	if time.Now().After(item.expiresAt) {
		c.remove(item)
		return nil, false
	}

	c.evictList.MoveToFront(item.element)
	return item.value, true
}

func (c *LRUCache) Set(key string, value interface{}) {
	c.mu.Lock()
	defer c.mu.Unlock()

	expiresAt := time.Now().Add(c.ttl)

	if existingItem, exists := c.items[key]; exists {
		existingItem.value = value
		existingItem.expiresAt = expiresAt
		c.evictList.MoveToFront(existingItem.element)
		return
	}

	if len(c.items) >= c.maxSize {
		c.evictLRU()
	}

	element := c.evictList.PushFront(key)
	item := &entry{
		key:       key,
		value:     value,
		expiresAt: expiresAt,
		element:   element,
	}
	c.items[key] = item
}

func (c *LRUCache) Remove(key string) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if item, exists := c.items[key]; exists {
		c.remove(item)
	}
}

func (c *LRUCache) Clear() {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.items = make(map[string]*entry)
	c.evictList = list.New()
}

func (c *LRUCache) Size() int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.items)
}

func (c *LRUCache) remove(item *entry) {
	delete(c.items, item.key)
	c.evictList.Remove(item.element)
}

func (c *LRUCache) evictLRU() {
	back := c.evictList.Back()
	if back != nil {
		key := back.Value.(string)
		if item, exists := c.items[key]; exists {
			c.remove(item)
		}
	}
}

func (c *LRUCache) Cleanup() {
	c.mu.Lock()
	defer c.mu.Unlock()

	now := time.Now()
	for key, item := range c.items {
		if now.After(item.expiresAt) {
			c.remove(item)
			delete(c.items, key)
		}
	}
}

