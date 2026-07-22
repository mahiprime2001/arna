// Arna services: accounts + social graph (Go + SQLite).
//
// Stores accounts, sessions, and friendships only. It never stores messages:
// chat is device-local and end-to-end encrypted, with a relay (added later)
// that forwards ciphertext and forgets it after delivery.
package main

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"golang.org/x/crypto/bcrypt"
	_ "modernc.org/sqlite"
)

var db *sql.DB

func main() {
	var err error
	db, err = sql.Open("sqlite", env("ARNA_DB", "arna.db"))
	if err != nil {
		log.Fatal(err)
	}
	db.SetMaxOpenConns(1) // SQLite: single writer
	migrate()

	mux := http.NewServeMux()
	mux.HandleFunc("/api/health", health)
	mux.HandleFunc("/api/signup", signup)
	mux.HandleFunc("/api/login", login)
	mux.HandleFunc("/api/logout", logout)
	mux.HandleFunc("/api/me", me)
	mux.HandleFunc("/api/presence/ping", presencePing)
	mux.HandleFunc("/api/friends", friendsList)
	mux.HandleFunc("/api/friends/request", friendRequest)
	mux.HandleFunc("/api/friends/respond", friendRespond)
	mux.HandleFunc("/api/friends/cancel", friendCancel)
	mux.HandleFunc("/api/friends/remove", friendRemove)
	mux.HandleFunc("/api/users/search", userSearch)
	mux.HandleFunc("/api/keys", setKeys)
	mux.HandleFunc("/ws", wsHandler)

	// Serve the built client (single-origin) when ARNA_WEB_DIR is set, with an
	// SPA fallback to index.html. Used by the Docker image / VPS deploy.
	if webDir := env("ARNA_WEB_DIR", ""); webDir != "" {
		mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
			p := filepath.Join(webDir, filepath.Clean("/"+r.URL.Path))
			if info, err := os.Stat(p); err == nil && !info.IsDir() {
				http.ServeFile(w, r, p)
				return
			}
			http.ServeFile(w, r, filepath.Join(webDir, "index.html"))
		})
	}

	addr := "0.0.0.0:" + env("PORT", "8787")
	cert, key := env("ARNA_TLS_CERT", ""), env("ARNA_TLS_KEY", "")
	if cert != "" && key != "" {
		log.Println("arna services listening (https/wss) on", addr)
		log.Fatal(http.ListenAndServeTLS(addr, cert, key, cors(mux)))
	}
	log.Println("arna services listening (http) on", addr)
	log.Fatal(http.ListenAndServe(addr, cors(mux)))
}

func env(k, d string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return d
}

func migrate() {
	db.Exec(`CREATE TABLE IF NOT EXISTS users (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		email TEXT UNIQUE NOT NULL,
		name TEXT NOT NULL,
		handle TEXT NOT NULL,
		pass_hash TEXT NOT NULL,
		last_seen TEXT,
		pubkey TEXT,
		created_at TEXT NOT NULL
	)`)
	db.Exec(`ALTER TABLE users ADD COLUMN last_seen TEXT`) // for older DBs; ignored if present
	db.Exec(`ALTER TABLE users ADD COLUMN pubkey TEXT`)
	db.Exec(`CREATE TABLE IF NOT EXISTS sessions (
		token TEXT PRIMARY KEY,
		user_id INTEGER NOT NULL,
		created_at TEXT NOT NULL
	)`)
	db.Exec(`CREATE TABLE IF NOT EXISTS friend_edges (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		requester_id INTEGER NOT NULL,
		addressee_id INTEGER NOT NULL,
		status TEXT NOT NULL,
		created_at TEXT NOT NULL,
		UNIQUE(requester_id, addressee_id)
	)`)
}

// Cross-origin dev: client (4320) calls this API (8787).
func cors(h http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		h.ServeHTTP(w, r)
	})
}

type User struct {
	ID     int64  `json:"id"`
	Email  string `json:"email"`
	Name   string `json:"name"`
	Handle string `json:"handle"`
}

func now() string { return time.Now().UTC().Format(time.RFC3339) }

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}

func fail(w http.ResponseWriter, code int, msg string) {
	writeJSON(w, code, map[string]string{"error": msg})
}

func newToken() string {
	b := make([]byte, 24)
	_, _ = rand.Read(b)
	return hex.EncodeToString(b)
}

func createSession(uid int64) (string, error) {
	t := newToken()
	_, err := db.Exec("INSERT INTO sessions(token,user_id,created_at) VALUES(?,?,?)", t, uid, now())
	return t, err
}

func bearer(r *http.Request) string {
	a := r.Header.Get("Authorization")
	if strings.HasPrefix(a, "Bearer ") {
		return a[7:]
	}
	return ""
}

func currentUID(r *http.Request) (int64, bool) {
	tok := bearer(r)
	if tok == "" {
		return 0, false
	}
	var uid int64
	if db.QueryRow("SELECT user_id FROM sessions WHERE token=?", tok).Scan(&uid) != nil {
		return 0, false
	}
	return uid, true
}

func presenceFor(ls sql.NullString) string {
	if !ls.Valid || ls.String == "" {
		return "offline"
	}
	t, err := time.Parse(time.RFC3339, ls.String)
	if err != nil || time.Since(t) > 45*time.Second {
		return "offline"
	}
	return "online"
}

func health(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

func signup(w http.ResponseWriter, r *http.Request) {
	var in struct{ Email, Name, Password string }
	if json.NewDecoder(r.Body).Decode(&in) != nil {
		fail(w, 400, "bad request")
		return
	}
	in.Email = strings.ToLower(strings.TrimSpace(in.Email))
	if in.Email == "" || len(in.Password) < 6 {
		fail(w, 400, "email and a 6+ character password are required")
		return
	}
	name := strings.TrimSpace(in.Name)
	if name == "" {
		name = strings.Split(in.Email, "@")[0]
	}
	handle := "@" + strings.Split(in.Email, "@")[0]
	hash, _ := bcrypt.GenerateFromPassword([]byte(in.Password), bcrypt.DefaultCost)
	res, err := db.Exec("INSERT INTO users(email,name,handle,pass_hash,last_seen,created_at) VALUES(?,?,?,?,?,?)",
		in.Email, name, handle, string(hash), now(), now())
	if err != nil {
		fail(w, 409, "that email is already registered")
		return
	}
	uid, _ := res.LastInsertId()
	tok, _ := createSession(uid)
	writeJSON(w, 200, map[string]any{"token": tok, "user": User{uid, in.Email, name, handle}})
}

func login(w http.ResponseWriter, r *http.Request) {
	var in struct{ Email, Password string }
	if json.NewDecoder(r.Body).Decode(&in) != nil {
		fail(w, 400, "bad request")
		return
	}
	in.Email = strings.ToLower(strings.TrimSpace(in.Email))
	var u User
	var hash string
	err := db.QueryRow("SELECT id,email,name,handle,pass_hash FROM users WHERE email=?", in.Email).
		Scan(&u.ID, &u.Email, &u.Name, &u.Handle, &hash)
	if err != nil || bcrypt.CompareHashAndPassword([]byte(hash), []byte(in.Password)) != nil {
		fail(w, 401, "wrong email or password")
		return
	}
	db.Exec("UPDATE users SET last_seen=? WHERE id=?", now(), u.ID)
	tok, _ := createSession(u.ID)
	writeJSON(w, 200, map[string]any{"token": tok, "user": u})
}

func me(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	var u User
	if db.QueryRow("SELECT id,email,name,handle FROM users WHERE id=?", uid).
		Scan(&u.ID, &u.Email, &u.Name, &u.Handle) != nil {
		fail(w, 401, "not signed in")
		return
	}
	writeJSON(w, 200, map[string]any{"user": u})
}

func logout(w http.ResponseWriter, r *http.Request) {
	db.Exec("DELETE FROM sessions WHERE token=?", bearer(r))
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

// setKeys registers the caller's public key so friends can encrypt to them.
// Only the public key is ever stored; private keys never leave the device.
func setKeys(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	var in struct{ Pubkey string }
	json.NewDecoder(r.Body).Decode(&in)
	if strings.TrimSpace(in.Pubkey) == "" {
		fail(w, 400, "missing pubkey")
		return
	}
	db.Exec("UPDATE users SET pubkey=? WHERE id=?", in.Pubkey, uid)
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

func presencePing(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	db.Exec("UPDATE users SET last_seen=? WHERE id=?", now(), uid)
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

// GET /api/friends -> { friends, incoming, outgoing }
func friendsList(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}

	friends := []map[string]any{}
	rows, _ := db.Query(`SELECT u.id,u.name,u.handle,u.last_seen,u.pubkey FROM friend_edges e
		JOIN users u ON u.id = CASE WHEN e.requester_id=? THEN e.addressee_id ELSE e.requester_id END
		WHERE e.status='accepted' AND (e.requester_id=? OR e.addressee_id=?)
		ORDER BY u.name`, uid, uid, uid)
	if rows != nil {
		for rows.Next() {
			var id int64
			var name, handle string
			var ls, pk sql.NullString
			rows.Scan(&id, &name, &handle, &ls, &pk)
			friends = append(friends, map[string]any{
				"id": id, "name": name, "handle": handle,
				"presence": presenceFor(ls), "pubkey": pk.String,
			})
		}
		rows.Close()
	}

	incoming := []map[string]any{}
	rows, _ = db.Query(`SELECT e.id,u.id,u.name,u.handle FROM friend_edges e
		JOIN users u ON u.id=e.requester_id
		WHERE e.addressee_id=? AND e.status='pending' ORDER BY e.created_at DESC`, uid)
	if rows != nil {
		for rows.Next() {
			var eid, userID int64
			var name, handle string
			rows.Scan(&eid, &userID, &name, &handle)
			incoming = append(incoming, map[string]any{
				"id": eid, "userId": userID, "name": name, "handle": handle,
			})
		}
		rows.Close()
	}

	outgoing := []map[string]any{}
	rows, _ = db.Query(`SELECT e.id,u.handle FROM friend_edges e
		JOIN users u ON u.id=e.addressee_id
		WHERE e.requester_id=? AND e.status='pending' ORDER BY e.created_at DESC`, uid)
	if rows != nil {
		for rows.Next() {
			var eid int64
			var handle string
			rows.Scan(&eid, &handle)
			outgoing = append(outgoing, map[string]any{"id": eid, "handle": handle})
		}
		rows.Close()
	}

	writeJSON(w, 200, map[string]any{"friends": friends, "incoming": incoming, "outgoing": outgoing})
}

func friendRequest(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	var in struct{ Handle, Email string }
	json.NewDecoder(r.Body).Decode(&in)
	q := strings.ToLower(strings.TrimSpace(in.Handle))
	if q == "" {
		q = strings.ToLower(strings.TrimSpace(in.Email))
	}
	if q == "" {
		fail(w, 400, "who do you want to add?")
		return
	}
	handle := "@" + strings.TrimPrefix(q, "@")

	var tid int64
	if db.QueryRow("SELECT id FROM users WHERE lower(handle)=? OR lower(email)=?", handle, q).Scan(&tid) != nil {
		fail(w, 404, "no one found with that handle or email")
		return
	}
	if tid == uid {
		fail(w, 400, "you can't add yourself")
		return
	}

	var eid, reqr int64
	var status string
	if db.QueryRow(`SELECT id,status,requester_id FROM friend_edges
		WHERE (requester_id=? AND addressee_id=?) OR (requester_id=? AND addressee_id=?)`,
		uid, tid, tid, uid).Scan(&eid, &status, &reqr) == nil {
		if status == "accepted" {
			fail(w, 409, "you're already friends")
			return
		}
		if reqr == tid { // they already asked you; accept it
			db.Exec("UPDATE friend_edges SET status='accepted' WHERE id=?", eid)
			writeJSON(w, 200, map[string]string{"status": "accepted"})
			return
		}
		fail(w, 409, "request already sent")
		return
	}

	db.Exec("INSERT INTO friend_edges(requester_id,addressee_id,status,created_at) VALUES(?,?,?,?)",
		uid, tid, "pending", now())
	writeJSON(w, 200, map[string]string{"status": "sent"})
}

func friendRespond(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	var in struct {
		ID     int64
		Action string
	}
	json.NewDecoder(r.Body).Decode(&in)
	var reqr int64
	if db.QueryRow("SELECT requester_id FROM friend_edges WHERE id=? AND addressee_id=? AND status='pending'",
		in.ID, uid).Scan(&reqr) != nil {
		fail(w, 404, "request not found")
		return
	}
	if in.Action == "accept" {
		db.Exec("UPDATE friend_edges SET status='accepted' WHERE id=?", in.ID)
	} else {
		db.Exec("DELETE FROM friend_edges WHERE id=?", in.ID)
	}
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

func friendCancel(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	var in struct{ ID int64 }
	json.NewDecoder(r.Body).Decode(&in)
	db.Exec("DELETE FROM friend_edges WHERE id=? AND requester_id=? AND status='pending'", in.ID, uid)
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

func friendRemove(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	var in struct{ UserID int64 }
	json.NewDecoder(r.Body).Decode(&in)
	db.Exec(`DELETE FROM friend_edges WHERE status='accepted'
		AND ((requester_id=? AND addressee_id=?) OR (requester_id=? AND addressee_id=?))`,
		uid, in.UserID, in.UserID, uid)
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

// GET /api/users/search?q= -> people you can add, with relationship status.
func userSearch(w http.ResponseWriter, r *http.Request) {
	uid, ok := currentUID(r)
	if !ok {
		fail(w, 401, "not signed in")
		return
	}
	q := strings.ToLower(strings.TrimSpace(r.URL.Query().Get("q")))
	if len(q) < 1 {
		writeJSON(w, 200, map[string]any{"users": []any{}})
		return
	}
	like := "%" + q + "%"
	// Collect rows and close BEFORE computing relStatus: SQLite is capped at one
	// connection, so a query inside an open rows loop would self-deadlock.
	type hit struct {
		id           int64
		name, handle string
	}
	hits := []hit{}
	rows, _ := db.Query(`SELECT id,name,handle FROM users
		WHERE id!=? AND (lower(handle) LIKE ? OR lower(name) LIKE ? OR lower(email) LIKE ?)
		ORDER BY name LIMIT 10`, uid, like, like, like)
	if rows != nil {
		for rows.Next() {
			var h hit
			rows.Scan(&h.id, &h.name, &h.handle)
			hits = append(hits, h)
		}
		rows.Close()
	}
	users := []map[string]any{}
	for _, h := range hits {
		users = append(users, map[string]any{
			"id": h.id, "name": h.name, "handle": h.handle, "status": relStatus(uid, h.id),
		})
	}
	writeJSON(w, 200, map[string]any{"users": users})
}

// none | friends | incoming | outgoing
func relStatus(me, other int64) string {
	var status string
	var reqr int64
	if db.QueryRow(`SELECT status,requester_id FROM friend_edges
		WHERE (requester_id=? AND addressee_id=?) OR (requester_id=? AND addressee_id=?)`,
		me, other, other, me).Scan(&status, &reqr) != nil {
		return "none"
	}
	if status == "accepted" {
		return "friends"
	}
	if reqr == me {
		return "outgoing"
	}
	return "incoming"
}
