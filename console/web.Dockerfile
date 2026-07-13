# Serve the Arna console as a static web app, with the backend URL baked in.
# Anyone can then open it in a browser to sign in and connect — no install.

# ---- build the Vue app ----
FROM node:20-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
# The server the hosted console talks to (baked at build time).
ARG VITE_ARNA_BACKEND=ws://187.124.99.4:48080/ws
ENV VITE_ARNA_BACKEND=$VITE_ARNA_BACKEND
RUN npm run build

# ---- serve the static files ----
FROM nginx:alpine
COPY --from=build /app/dist /usr/share/nginx/html
# SPA fallback so deep links (?connect=…) work.
RUN printf 'server {\n  listen 80;\n  location / {\n    root /usr/share/nginx/html;\n    try_files $uri $uri/ /index.html;\n  }\n}\n' \
    > /etc/nginx/conf.d/default.conf
EXPOSE 80
