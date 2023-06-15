# export TURBOFISH_ROOT in you shell configuration file to execute this Makefile independently
include $(TURBOFISH_ROOT)/boilerplates.mk

arch := ivybridge-cpu
target := $(arch)
custom_target := $(arch).json
ifeq ($(DEBUG),yes)
	rust_archive = target/$(target)/debug/lib$(LIBMOD).a
else
	optim_level += --release
	rust_archive = target/$(target)/release/lib$(LIBMOD).a
endif

.PHONY = all clean fclean re exec build_rust

all: .rust-toolchain build_rust

.rust-toolchain: rust-toolchain
	rustup component add rust-src
	rustup component add rustfmt
	echo "Updated" > .rust-toolchain

build_rust:
	cargo build -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem $(optim_level) --target $(custom_target) --verbose
	make -C dummy
	make -C rtc
	make -C key
	make -C syslog
	make -C shell

clean:
	cargo clean
	make -C dummy clean
	make -C rtc clean
	make -C key clean
	make -C syslog clean
	make -C shell clean

fclean: clean

re: fclean all
