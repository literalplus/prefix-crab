# GNS3 network simulation
USER := lit
BRIDGE_IF := brgns3
ULA_PFX := fddc:9d0b:e318
ULA_PFX_CIDR := $(ULA_PFX)::/48
ROUTER := $(ULA_PFX):8710::bb:1
MY_ADDR := $(ULA_PFX):8710::cc:1

# bash colors
BOLD := $$(tput setaf 6; tput bold)
UNBOLD := $$(tput sgr0)


.PHONY: gns3
gns3:
	sudo systemctl start gns3-server@$(USER)
	sudo ip link add name $(BRIDGE_IF) type bridge
	sudo ip link set dev $(BRIDGE_IF) up
	sudo ip -6 addr add $(MY_ADDR)/64 dev $(BRIDGE_IF)
	@read -p "#### Open the GNS3 UI and start all devices of the project now (green play button), then press any key."
	sudo ip -6 route add $(ULA_PFX_CIDR) dev $(BRIDGE_IF) via $(ROUTER) metric 3
	ip -6 route | grep $(ULA_PFX_CIDR) || echo "route adding failed"

.PHONY: gns3-down
gns3-down:
	sudo ip -6 route del $(ULA_PFX_CIDR) || echo "route removal failed, was it present?"
	sudo ip link del $(BRIDGE_IF) || echo "link removal failed, was it present?"
	sudo systemctl stop gns3-server@$(USER)

.PHONY: build
build:
	cargo build

.PHONY: build-release
build-release:
	cargo build --release

.PHONY: clippy
clippy:
	cargo clippy --workspace

.PHONY: run-zmap
run-zmap:
	cd zmap-buddy && cargo run -- rabbit-mq-listen

.PHONY: run-aggregator
run-aggregator:
	cd aggregator && cargo run

.PHONY: example-scan
example-scan:
	./scan-oneoff.sh fddc:9d0b:e318:8712::bc:1/48

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

.PHONY: tmux
tmux:
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
	@tmux new-window -n agg -d make --no-print-directory run-aggregator || echo "aggregator already running."
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
	@tmux kill-session -t pfx-crab

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
