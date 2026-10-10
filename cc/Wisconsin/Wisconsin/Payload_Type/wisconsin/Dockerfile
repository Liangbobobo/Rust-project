FROM python:3.11-bookworm

# Install build tools, mingw-w64 cross-compiler for Windows targets, and dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    build-essential \
    gcc-mingw-w64-x86-64 \
    libssl-dev \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Rust 1.94 (edition 2024 target)
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.94.0 \
    && rustup target add x86_64-pc-windows-gnu

# Install Mythic container framework
RUN pip3 install --no-cache-dir "mythic-container>=0.5.21"

# Container working directory
WORKDIR /Mythic

# Copy container service code
COPY . /Mythic

CMD ["python3", "main.py"]
