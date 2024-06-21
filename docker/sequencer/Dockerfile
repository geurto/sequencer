ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# Avoid prompts from apt
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
  apt-get install -y --no-install-recommends \
  curl \
  libasound2-dev \
  nano \
  && \
  rm -rf /var/lib/apt/lists/* && \
  apt-get autoremove -y && \
  apt-get clean

RUN git clone https://github.com/cjbarnes18/ttymidi.git && \
  cd ttymidi && \
  make && \
  make install && \
  cd .. && \
  rm -rf ttymidi

RUN mkdir -p /home/sequencer
COPY src/ /home/sequencer/src
COPY Cargo.toml /home/sequencer/Cargo.toml

# Set the working directory
WORKDIR /home/sequencer

# Install the necessary dependencies
RUN cargo build --release

COPY init.sh /home/sequencer/init.sh
