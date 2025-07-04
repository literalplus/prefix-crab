# GNS3 network simulation
USER := lit
BRIDGE_IF := brgns3
ULA_PFX := fddc:9d0b:e318
ULA_PFX_CIDR := $(ULA_PFX)::/48
ROUTER := $(ULA_PFX):8710::bb:1
MY_ADDR_ZMAP := $(ULA_PFX):8710::cc:1
MY_ADDR_YARRP := $(ULA_PFX):8710::cc:2

# bash colors
BOLD := $$(tput setaf 6; tput bold)
UNBOLD := $$(tput sgr0)

# Docker
DOCKER_PREFIX := prefix-crab.local

# --- Local setup
.PHONY: gns3
gns3:
	sudo systemctl start gns3-server@$(USER)
	sudo ip link add name $(BRIDGE_IF) type bridge
	sudo ip link set dev $(BRIDGE_IF) up
	sudo ip -6 addr add $(MY_ADDR_ZMAP)/64 dev $(BRIDGE_IF)
	sudo ip -6 addr add $(MY_ADDR_YARRP)/64 dev $(BRIDGE_IF)
# Project starts automatically with the server if you set it up as such in File -> Edit project
#@read -p "#### Open the GNS3 UI and start all devices of the project now (green play button), then press any key."
	sudo ip -6 route add $(ULA_PFX_CIDR) dev $(BRIDGE_IF) via $(ROUTER) metric 3
	ip -6 route | grep $(ULA_PFX_CIDR) || echo "route adding failed"

.PHONY: gns3-down
gns3-down:
	sudo ip -6 route del $(ULA_PFX_CIDR) || echo "route removal failed, was it present?"
	sudo ip link del $(BRIDGE_IF) || echo "link removal failed, was it present?"
	sudo systemctl stop gns3-server@$(USER)

.PHONY: infra
infra: docker-running
	@if ! docker-compose ps >/dev/null; then docker-compose up -d; fi

.PHONY: docker-running
docker-running:
	@if ! systemctl is-active docker >/dev/null; then sudo systemctl start docker; fi

.PHONY: rmq-ui
rmq-ui: infra
	@xdg-open http://10.45.87.51:15672/
	@echo "#### Credentials: rabbit / localsetupveryinsecure"

# --- Build
.PHONY: build
build:
	cargo build --workspace

.PHONY: build-release
build-release:
	cargo build --release --workspace

.PHONY: clippy
clippy:
	cargo clippy --workspace

# --- Run local components
.PHONY: run-zmap
run-zmap:
	cd zmap-buddy && cargo run

.PHONY: run-aggregator
run-aggregator:
	cd aggregator && cargo run

.PHONY: run-yarrp
run-yarrp:
	cd yarrp-buddy && cargo run

.PHONY: run-guard
run-guard:
	cd seed-guard && cargo run

.PHONY: example-scan
example-scan:
	./scan-oneoff.sh fddc:9d0b:e318:8712::bc:1/48

# --- tmux local setup
.PHONY: tmux
tmux:
	@if which resize >/dev/null 2>&1; then resize -s 30 129; fi
	tmux new -A -s pfx-crab make --no-print-directory in-tmux

.PHONY: in-tmux
in-tmux: 
	@tmux set -g remain-on-exit failed
	@tmux bind-key r "run-shell 'kill #{pane_pid}'; respawn-pane -k"
	@tmux bind-key e "display-popup -T 'Example Scan' -EE './scan-oneoff.sh fddc:9d0b:e318:8712::bc:1/48 && sleep 1';"
	@tmux bind-key t "display-popup -T 'Custom Scan' -EE './scan-oneoff.sh && sleep 3';"
	@tmux set status-bg lightblue
	@tmux set -g window-status-current-style bg=green
	@if ! [[ -f .env ]]; then make --no-print-directory configure-env; fi
	@make --no-print-directory infra
	@if ! ip link show brgns3 >/dev/null 2>&1; then make --no-print-directory gns3; fi
	@tmux new-window -n zmap -d make --no-print-directory run-zmap || echo "zmap-buddy already running."
	@tmux new-window -n yrp -d make --no-print-directory run-yarrp || echo "yarrp-buddy already running."
	@tmux new-window -n agg -d make --no-print-directory run-aggregator || echo "aggregator already running."
	@tmux new-window -n seed -d make --no-print-directory run-guard || echo "seed-guard already running."
	@clear
	@make -s banner
	@echo "    $(BOLD)Trigger an example scan:$(UNBOLD)    Ctrl-B, then E"
	@echo "    $(BOLD)Trigger a custom scan:$(UNBOLD)      Ctrl-B, then T    (e.g. $(ULA_PFX_CIDR))"
	@echo "    $(BOLD)Restart a window:$(UNBOLD)           Ctrl-B, then R"
	@echo "    $(BOLD)Open RabbitMQ UI:$(UNBOLD)           \`make rmq-ui\`"
	@echo ""
	@echo "    When finished, press $(BOLD)Ctrl-D$(UNBOLD) in this window."
	@echo ""
	@if which zsh >/dev/null; then zsh; else bash; fi || true
	@tmux set -g remain-on-exit off
	@for _pane in $$(tmux list-panes -s -F '#I.#P'); do tmux send-keys -t $${_pane} C-c; done

.PHONY: configure-env
configure-env:
	@if [[ ! -f .env ]]; then cp .env.template .env; echo "New environment config at \`.env\`"; fi
	@read -p "#### Press any key to edit your environment configuration."
	@$(EDITOR) .env

.PHONY: banner
banner:
	@echo ""
	@echo "    ▄███████▄    ▄████████    ▄████████    ▄████████  ▄█  ▀████    ▐████▀       ▄████████    ▄████████    ▄████████ ▀█████████▄  ";
	@echo "   ███    ███   ███    ███   ███    █▀    ███    █▀  ███▌    ███  ▐███         ███    █▀    ███    ███   ███    ███   ███    ███ ";
	@echo "   ███    ███   ███    ███   ███    ███   ███    ███ ███    ███▌   ████▀       ███    ███   ███    ███   ███    ███   ███    ███ ";
	@echo "   ███    ███  ▄███▄▄▄▄██▀  ▄███▄▄▄      ▄███▄▄▄     ███▌    ▀███▄███▀         ███         ▄███▄▄▄▄██▀   ███    ███  ▄███▄▄▄██▀  ";
	@echo " ▀█████████▀  ▀▀███▀▀▀▀▀   ▀▀███▀▀▀     ▀▀███▀▀▀     ███▌    ████▀██▄          ███        ▀▀███▀▀▀▀▀   ▀███████████ ▀▀███▀▀▀██▄  ";
	@echo "   ███        ▀███████████   ███    █▄    ███        ███    ▐███  ▀███         ███    █▄  ▀███████████   ███    ███   ███    ██▄ ";
	@echo "   ███          ███    ███   ███    ███   ███        ███   ▄███     ███▄       ███    ███   ███    ███   ███    ███   ███    ███ ";
	@echo "  ▄████▀        ███    ███   ██████████   ███        █▀   ████       ███▄      ████████▀    ███    ███   ███    █▀  ▄█████████▀  ";
	@echo "                ███    ███                                                                  ███    ███                           ";
	@echo "                                                                                                                                ";

# --- Docker builds

.PHONY: docker-builder
docker-builder:
	docker build -t $(DOCKER_PREFIX)/builder .

.PHONY: buildah-builder
buildah-builder:
	buildah build -t $(DOCKER_PREFIX)/builder .
