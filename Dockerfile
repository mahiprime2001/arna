# Arna, one image: builds the React client and the Go backend, then serves the
# client + API + WebSocket relay from a single origin/port. Pure-Go SQLite, so
# no cgo. Intended for the VPS deploy behind Cloudflare (which provides HTTPS).

# ---- build the client ----
FROM node:20-alpine AS client
WORKDIR /client
COPY client/package*.json ./
RUN npm ci
COPY client/ ./
RUN npm run build          # -> /client/dist (same-origin API by default)

# ---- build the backend ----
FROM golang:1.26-alpine AS server
WORKDIR /src
COPY services/go.mod services/go.sum ./
RUN go mod download
COPY services/ ./
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -o /arna-services .

# ---- final image ----
FROM alpine:3.20
RUN adduser -D -u 10001 arna
WORKDIR /app
COPY --from=server /arna-services /app/arna-services
COPY --from=client /client/dist /app/web
RUN mkdir -p /data && chown -R arna /data /app
ENV ARNA_WEB_DIR=/app/web \
    ARNA_DB=/data/arna.db \
    PORT=8080
USER arna
EXPOSE 8080
CMD ["/app/arna-services"]
