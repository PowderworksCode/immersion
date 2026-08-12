# powderman as a container, for Fly (production and PR previews).
#
# A preview has no herdr and no systemd — the daemon reaches for both but treats
# their absence as "unreachable", so the workbench UI, the command bus, the
# palette, the widgets and (on branches that have it) the MCP server all work;
# only the fleet and run execution go dark. That is exactly what a preview is
# for: driving the interface before it merges.

# --- build ------------------------------------------------------------------
FROM rust:1-bookworm AS build
WORKDIR /src
# rusqlite's `bundled` feature compiles SQLite from C, so a C toolchain is
# needed at build time.
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential pkg-config \
 && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p powderman

# --- run --------------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/powderman /usr/local/bin/powderman

# Fly routes to internal_port 8080; the DB lives on the ephemeral container fs
# (a preview keeps no state); schedules are OFF (a preview must never fire the
# 06:00 sweep); the MCP host guard is open because Fly reaches the app under its
# own *.fly.dev hostname, not localhost.
ENV POWDERMAN_PORT=8080 \
    POWDERMAN_DB=/data/powderman.db \
    POWDERMAN_SCHEDULES=0 \
    POWDERMAN_MCP_ALLOWED_HOSTS=*
RUN mkdir -p /data
EXPOSE 8080
CMD ["powderman"]
