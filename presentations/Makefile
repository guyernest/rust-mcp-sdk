# Presentation Makefile

.PHONY: serve open help

# Default port
PORT ?= 8000

help:
	@echo "cargo-pmcp Presentation"
	@echo ""
	@echo "Usage:"
	@echo "  make serve    - Start local server on port $(PORT)"
	@echo "  make open     - Open presentation in browser"
	@echo ""
	@echo "Keyboard shortcuts in presentation:"
	@echo "  Space/Arrow   - Navigate slides"
	@echo "  S             - Speaker notes view"
	@echo "  F             - Fullscreen"
	@echo "  O             - Overview mode"
	@echo "  Esc           - Exit fullscreen/overview"

serve:
	@echo "Starting server at http://localhost:$(PORT)"
	@echo "Open: http://localhost:$(PORT)/cargo-pmcp-demo.html"
	@echo ""
	@python3 -m http.server $(PORT)

open:
	@open cargo-pmcp-demo.html || xdg-open cargo-pmcp-demo.html 2>/dev/null || echo "Please open cargo-pmcp-demo.html in your browser"
