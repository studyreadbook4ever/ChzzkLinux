# Copyright 2026 studyreadbook4ever
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.


#!/bin/bash
# 코어 격리가 없는 버전의 launcher(Powered by Gemini 3)

# 1. sudo 권한 확인
if [ "$EUID" -ne 0 ]; then
  echo "❌ 이 스크립트는 sudo 권한으로 실행해야 합니다. (예: sudo ./.launcher.sh 랄로 1080)"
  exit 1
fi

if [ "$#" -lt 1 ]; then
    echo "사용법: sudo ./.launcher.sh <채널명(channels.toml 기준)> [화질(기본:1080)]"
    exit 1
fi

CHANNEL=$1
QUALITY=${2:-1080}
REAL_USER=${SUDO_USER:-$USER}

# 2. 컴파일된 바이너리 경로 탐색 (오류 메시지 숨김 처리)
BIN_PATH=$(find ./target/release -maxdepth 1 -type f -executable 2>/dev/null | grep -v '\.d$' | head -n 1)

# =====================================================================
# 3. OS 감지 및 의존성 라이브러리 자동 설치 (최초 1회 셋업)
# =====================================================================
# 핵심 패키지(mpv)가 없거나, cargo가 없거나, 빌드된 실행 파일이 없다면 셋업 모드 가동
if ! command -v mpv &> /dev/null || ! sudo -u "$REAL_USER" bash -c "source \$HOME/.cargo/env 2>/dev/null || true; command -v cargo" &> /dev/null || [ -z "$BIN_PATH" ]; then
    echo "==================================================="
    echo " 🛠️ 초기 환경 셋업이 필요합니다. 필수 라이브러리를 설치합니다..."
    echo "==================================================="
    
    if [ -f /etc/os-release ]; then
        source /etc/os-release
        OS_ID=$ID
        OS_LIKE=$ID_LIKE
    else
        echo "❌ OS를 감지할 수 없습니다."
        exit 1
    fi

    # OS별 패키지 매니저 분기 (요청하신 로직 탑재)
    if [[ "$OS_ID" == "ubuntu" || "$OS_LIKE" == *"ubuntu"* || "$OS_ID" == "debian" || "$OS_LIKE" == *"debian"* ]]; then
        echo "[Ubuntu / Debian 계열] apt로 패키지를 설치합니다."
        apt-get update
        apt-get install -y curl gcc pkg-config libssl-dev mpv pipewire
    elif [[ "$OS_ID" == "fedora" || "$OS_LIKE" == *"fedora"* || "$OS_LIKE" == *"rhel"* ]]; then
        echo "[Fedora / RHEL 계열] dnf로 패키지를 설치합니다."
        dnf install -y curl gcc pkgconf-pkg-config openssl-devel mpv pipewire
    elif [[ "$OS_ID" == "arch" || "$OS_LIKE" == *"arch"* ]]; then
        echo "[Arch Linux 계열] pacman으로 패키지를 설치합니다."
        # --needed 옵션: 이미 설치된 패키지는 무시하고 건너뛰어 속도를 비약적으로 높임
        pacman -Sy --needed --noconfirm curl gcc pkgconf openssl mpv pipewire
    else
        echo "❌ 지원하지 않는 리눅스 배포판입니다 ($OS_ID)."
        exit 1
    fi

    # 일반 유저 권한으로 Rust 툴체인이 없으면 자동 설치
    if ! sudo -u "$REAL_USER" bash -c "source \$HOME/.cargo/env 2>/dev/null || true; command -v cargo" &> /dev/null; then
        echo "🦀 Rust 툴체인 설치 중..."
        sudo -u "$REAL_USER" curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sudo -u "$REAL_USER" sh -s -- -y
    fi

    # 빌드된 바이너리가 없으면 새로 컴파일
    if [ -z "$BIN_PATH" ]; then
        echo "Rust 프로젝트 빌드 중... (최초 1회만 진행되므로 약간의 시간이 소요됩니다)"
        # root로 빌드하여 폴더 권한이 꼬이는 것을 막기 위해 일반 유저로 권한을 내려서 빌드
        sudo -i -u "$REAL_USER" bash -c "cd '$(pwd)' && source \$HOME/.cargo/env 2>/dev/null || true && cargo build --release"
        
        # 빌드 후 다시 바이너리 경로 찾기
        BIN_PATH=$(find ./target/release -maxdepth 1 -type f -executable 2>/dev/null | grep -v '\.d$' | head -n 1)
    fi
    echo "✅ 필수 라이브러리 설치 및 셋업 완료!"
    echo "==================================================="
fi

if [ -z "$BIN_PATH" ] || [ ! -f "$BIN_PATH" ]; then
    echo "❌ 빌드에 실패했습니다. Rust 소스 코드나 환경을 확인해주세요."
    exit 1
fi

# =====================================================================
# 4. 실제 뷰어 실행 로직
# =====================================================================
echo "==================================================="
echo " 치지직 뷰어 실행기"
echo " 대상 채널: $CHANNEL / 해상도: ${QUALITY}p"
echo " 코어 격리: X"
echo "==================================================="

USER_UID=$(id -u "$REAL_USER")

# sudo 환경에서도 GUI와 사운드 출력이 막히지 않도록 일반 유저로 강등시켜서 실행 준비
export XDG_RUNTIME_DIR="/run/user/$USER_UID"
export WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-"wayland-0"}
export DISPLAY=${DISPLAY:-":0"}
export PULSE_SERVER="unix:/run/user/$USER_UID/pulse/native"

export PIPEWIRE_RUNTIME_DIR="/run/user/$USER_UID"

# 일반적인 하나의 프로세스로써 방송 프로그램 실행하는 무난한 버전의 launcher입니다. 최종적으로 컴파일된 
# -E 옵션으로 위에서 세팅한 환경 변수를 유지한 채, -u 옵션으로 원래 사용자 권한으로 바이너리를 실행합니다.
sudo -E -u "$REAL_USER" "$BIN_PATH" "$CHANNEL" "$QUALITY"
