.DEFAULT_GOAL := install

ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
CARGO_TARGET_DIR ?= $(ROOT)/desktop/src-tauri/target
APP := $(CARGO_TARGET_DIR)/release/bundle/macos/Mind.app
INSTALL_DIR := $(HOME)/Applications
LSREGISTER := /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

# bootstrap installs bun, node and rust into the home directory; make sure later steps see them
export PATH := $(HOME)/.local/bin:$(HOME)/.bun/bin:$(HOME)/.cargo/bin:$(PATH)

# 依赖安装交给 Bun：同样的锁文件，安装速度和内存都明显优于 npm ci。
# 脚本仍然用 npm 跑：vite / tauri 的 bin 是 node shebang，运行时没换，只是装包换人。
# Windows 上 bun.exe 可直接执行，不需要下面 NPM 那种 sh 兜底。
BUN = bun

# Windows 上 node 自带的 npm 没有扩展名，是个 bash 脚本（shebang: #!/usr/bin/env bash）。
# 某些 POSIX 层（比如 cygwin）无法直接 exec 它，会报
# "/cygdrive/.../npm: No such file or directory" —— 这是 execve 找不到解释器，
# 不是文件不存在。交给 sh 去读文件执行就跳过了 shebang 查找。
# macOS / Linux 没有这个问题，仍直接调用 npm。
#
# 局限：这里只覆盖官方安装包带的那个无扩展名脚本。nvm-windows、scoop 这类
# 只提供 npm.cmd 的环境不在覆盖范围内。
ifeq ($(OS),Windows_NT)
NPM = sh "$$(command -v npm)"
else
NPM = npm
endif

.PHONY: help bootstrap bootstrap-node web desktop dev install open

help:
	@printf '%s\n' \
		'make             check the toolchain, build Mind.app, install it to ~/Applications' \
		'make web         run the web app at http://localhost:8080' \
		'make desktop     build Mind.app without installing' \
		'make dev         run the desktop shell against the local web app' \
		'make open        open ~/Applications/Mind.app' \
		'make bootstrap   only check/install the toolchain, do not build'

# Check the toolchain only, installing whatever is missing
bootstrap:
	@sh scripts/bootstrap.sh rust

bootstrap-node:
	@sh scripts/bootstrap.sh node

# 安装不再需要 npm 的 --no-audit / --loglevel 参数：Bun 默认只输出结果，
# 被拦下的生命周期脚本用 `bun pm untrusted` 单独查看。
#
# 只认 lockfile：改 package.json 的脚本之类不该触发整包重装。
# --frozen-lockfile 让 bun 在 package.json 与 bun.lock 不一致时直接失败，
# 不会悄悄装出和仓库不同的版本。
#
# bootstrap-node 是伪目标，每次都会"更新"；当普通前置依赖会让这条规则永远重跑
# bun install。改成 order-only，只保证顺序，不参与时间戳比较。
web/node_modules: web/bun.lock | bootstrap-node
	@printf '  web: installing dependencies...\n'
	@cd web && $(BUN) install --frozen-lockfile
	@touch $@

desktop/node_modules: desktop/bun.lock | bootstrap-node
	@printf '  desktop: installing dependencies...\n'
	@cd desktop && $(BUN) install --frozen-lockfile
	@touch $@

web: web/node_modules
	cd web && $(NPM) run serve

desktop: bootstrap web/node_modules desktop/node_modules
	@# 构建产物里也有一份 Mind.app，Spotlight 会把它当成第二个应用；让 Spotlight 跳过 target/
	@mkdir -p desktop/src-tauri/target && touch desktop/src-tauri/target/.metadata_never_index
	cd desktop && $(NPM) run build

dev: bootstrap web/node_modules desktop/node_modules
	cd desktop && $(NPM) run dev

install: desktop
	mkdir -p "$(INSTALL_DIR)"
	rm -rf "$(INSTALL_DIR)/Mind.app"
	ditto "$(APP)" "$(INSTALL_DIR)/Mind.app"
	@# 构建产物里的那份副本会被 LaunchServices 和 Spotlight 一并收进去，
	@# "打开方式" 里就会出现两个 Mind。装完直接删掉，并解除注册。
	@-$(LSREGISTER) -u "$(APP)" >/dev/null 2>&1 || true
	@rm -rf "$(APP)"
	@printf 'Installed %s/Mind.app\n' "$(INSTALL_DIR)"

open:
	open "$(INSTALL_DIR)/Mind.app"
