ARG BASE_IMAGE=rust
FROM ${BASE_IMAGE}

# Avoid prompts from apt
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
  apt-get install -y --no-install-recommends \
  curl \
  libasound2-dev \
  logrotate \
  nano \
  openssh-server \
  && \
  rm -rf /var/lib/apt/lists/* && \
  apt-get autoremove -y && \
  apt-get clean

RUN mkdir -p /home/sequencer
COPY src/ /home/sequencer/src
COPY Cargo.toml /home/sequencer/Cargo.toml

# Set the working directory
WORKDIR /home/sequencer

# Install the necessary dependencies
RUN #cargo build --release

COPY init.sh /home/sequencer/init.sh
COPY src/entrypoint.sh /entrypoint.sh

RUN mkdir /var/run/sshd && \
    echo 'root:BigJuice420' | chpasswd && \
    sed -i 's|#PermitRootLogin prohibit-password|PermitRootLogin yes|' /etc/ssh/sshd_config && \
    sed -i 's|#PasswordAuthentication yes|PasswordAuthentication yes|' /etc/ssh/sshd_config && \
    sed -i 's|#PermitUserEnvironment no|PermitUserEnvironment yes|' /etc/ssh/sshd_config

#ENTRYPOINT ["/entrypoint.sh"]
CMD ["sleep", "infinity"]