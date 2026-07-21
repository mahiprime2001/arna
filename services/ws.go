package main

import (
	"encoding/json"
	"net/http"
	"sync"
	"sync/atomic"

	"github.com/gorilla/websocket"
)

var msgSeq int64

// The relay forwards end-to-end encrypted envelopes between devices. It cannot
// read them (they are ciphertext), and it never persists them: if the recipient
// is offline the envelope waits in memory only, and is dropped once delivered.

var upgrader = websocket.Upgrader{
	CheckOrigin: func(_ *http.Request) bool { return true }, // dev
}

type wsClient struct {
	uid  int64
	send chan []byte
}

var (
	hubMu   sync.Mutex
	clients = map[int64]map[*wsClient]bool{} // uid -> connected devices
	pending = map[int64][][]byte{}           // uid -> queued envelopes (in-memory)
)

func wsHandler(w http.ResponseWriter, r *http.Request) {
	tok := r.URL.Query().Get("token")
	if tok == "" {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	var uid int64
	if db.QueryRow("SELECT user_id FROM sessions WHERE token=?", tok).Scan(&uid) != nil {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}

	c := &wsClient{uid: uid, send: make(chan []byte, 64)}
	register(c)

	// writer
	go func() {
		for msg := range c.send {
			if conn.WriteMessage(websocket.TextMessage, msg) != nil {
				break
			}
		}
		conn.Close()
	}()

	// reader (blocks until the socket closes)
	for {
		_, data, err := conn.ReadMessage()
		if err != nil {
			break
		}
		var in struct {
			To         int64  `json:"to"`
			Nonce      string `json:"nonce"`
			Ciphertext string `json:"ciphertext"`
			Ts         int64  `json:"ts"`
		}
		if json.Unmarshal(data, &in) != nil || in.To == 0 {
			continue
		}
		out, _ := json.Marshal(map[string]any{
			"type": "msg", "id": atomic.AddInt64(&msgSeq, 1),
			"from": c.uid, "to": in.To,
			"nonce": in.Nonce, "ciphertext": in.Ciphertext, "ts": in.Ts,
		})
		deliver(in.To, out)
	}

	unregister(c)
}

func register(c *wsClient) {
	hubMu.Lock()
	if clients[c.uid] == nil {
		clients[c.uid] = map[*wsClient]bool{}
	}
	clients[c.uid][c] = true
	queued := pending[c.uid]
	delete(pending, c.uid) // deliver-then-forget
	hubMu.Unlock()

	for _, m := range queued {
		select {
		case c.send <- m:
		default:
		}
	}
}

func unregister(c *wsClient) {
	hubMu.Lock()
	if set := clients[c.uid]; set != nil {
		delete(set, c)
		if len(set) == 0 {
			delete(clients, c.uid)
		}
	}
	hubMu.Unlock()
	close(c.send)
}

func deliver(to int64, msg []byte) {
	hubMu.Lock()
	defer hubMu.Unlock()
	set := clients[to]
	if len(set) > 0 {
		for c := range set {
			select {
			case c.send <- msg:
			default: // slow client; skip this device
			}
		}
		return
	}
	// Offline: hold in memory until they connect, then it's dropped. Cap to
	// avoid unbounded growth.
	q := append(pending[to], msg)
	if len(q) > 500 {
		q = q[len(q)-500:]
	}
	pending[to] = q
}
