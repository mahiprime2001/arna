// Arna services: a small accounts backend (Go + SQLite).
//
// It stores ONLY accounts and sessions. It never stores messages: chat is
// device-local and end-to-end encrypted, with the relay (added later) only
// forwarding ciphertext and forgetting it after delivery.
package main

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"log"
	"net/http"
	"os"
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
	if err := migrate(); err != nil {
		log.Fatal(err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/api/health", health)
	mux.HandleFunc("/api/signup", signup)
	mux.HandleFunc("/api/login", login)
	mux.HandleFunc("/api/logout", logout)
	mux.HandleFunc("/api/me", me)

	addr := "0.0.0.0:" + env("PORT", "8787")
	log.Println("arna services listening on", addr)
	log.Fatal(http.ListenAndServe(addr, cors(mux)))
}

func env(k, d string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return d
}

func migrate() error {
	if _, err := db.Exec(`CREATE TABLE IF NOT EXISTS users (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		email TEXT UNIQUE NOT NULL,
		name TEXT NOT NULL,
		handle TEXT NOT NULL,
		pass_hash TEXT NOT NULL,
		created_at TEXT NOT NULL
	)`); err != nil {
		return err
	}
	_, err := db.Exec(`CREATE TABLE IF NOT EXISTS sessions (
		token TEXT PRIMARY KEY,
		user_id INTEGER NOT NULL,
		created_at TEXT NOT NULL
	)`)
	return err
}

// Cross-origin dev: the client (port 4320) calls this API (port 8787).
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
	_, err := db.Exec("INSERT INTO sessions(token,user_id,created_at) VALUES(?,?,?)",
		t, uid, time.Now().UTC().Format(time.RFC3339))
	return t, err
}

func bearer(r *http.Request) string {
	a := r.Header.Get("Authorization")
	if strings.HasPrefix(a, "Bearer ") {
		return a[7:]
	}
	return ""
}

func health(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, 200, map[string]string{"status": "ok"})
}

func signup(w http.ResponseWriter, r *http.Request) {
	var in struct {
		Email, Name, Password string
	}
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
	res, err := db.Exec("INSERT INTO users(email,name,handle,pass_hash,created_at) VALUES(?,?,?,?,?)",
		in.Email, name, handle, string(hash), time.Now().UTC().Format(time.RFC3339))
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
	tok, _ := createSession(u.ID)
	writeJSON(w, 200, map[string]any{"token": tok, "user": u})
}

func me(w http.ResponseWriter, r *http.Request) {
	tok := bearer(r)
	if tok == "" {
		fail(w, 401, "not signed in")
		return
	}
	var uid int64
	if db.QueryRow("SELECT user_id FROM sessions WHERE token=?", tok).Scan(&uid) != nil {
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
