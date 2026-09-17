ARG BASE_IMAGE=rust
FROM ${BASE_IMAGE}

# Avoid prompts from apt
ENV DEBIAN_FRONTEND=noninteractive

# alsa-sys (midir) and x11 (device_query) locate their libraries through
# pkg-config; the rest are what iced needs on a Linux host.
RUN apt-get update && \
  apt-get install -y --no-install-recommends \
  curl \
  nano \
  pkg-config \
  libasound2-dev \
  libx11-dev \
  libxcb1-dev \
  libxkbcommon-dev \
  && \
  rm -rf /var/lib/apt/lists/* && \
  apt-get autoremove -y && \
  apt-get clean

WORKDIR /home/sequencer

# Cargo.lock is copied too so the image builds the same dependency versions the
# repository was tested against.
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Deliberately not built here. The arm64 image is assembled under QEMU, where
# compiling the iced dependency tree takes long enough to be impractical; build
# inside the running container instead:
#
#     docker compose exec sequencer cargo build --release

COPY init.sh ./init.sh
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh ./init.sh

CMD ["sleep", "infinity"]
