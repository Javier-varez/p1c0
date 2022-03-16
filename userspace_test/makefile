THIS_FILE := $(firstword $(MAKEFILE_LIST))

CROSS_COMPILE ?= aarch64-none-elf-

CXX := $(CROSS_COMPILE)g++
LD := $(CROSS_COMPILE)ld
OBJCOPY := $(CROSS_COMPILE)objcopy

SOURCES := \
	src/main.cpp \
	src/relocations.cpp \
	src/syscalls.cpp

LINKER_SCRIPT := p1c0.ld

CXX_FLAGS := \
	-Iinclude \
	-nostdlib \
	-nodefaultlibs \
	-fno-rtti \
	-fno-exceptions \
	-fPIE \
	-O0 \
	-g3 \
	-std=gnu++17

LD_FLAGS := \
	-nostdlib \
	-no-dynamic-linker \
	-static \
	-pie \
	-T $(LINKER_SCRIPT)

OBJECTS := $(patsubst src/%.cpp, build/%.o, $(SOURCES))

TARGET := build/userspace_test
TARGET_BIN := $(TARGET).bin
TARGET_TEXT := $(TARGET)_text.bin
TARGET_RODATA := $(TARGET)_rodata.bin
TARGET_DATA := $(TARGET)_data.bin

ifeq ($(filter verbose, $(MAKECMDGOALS)), )
SILENT := @
else
SILENT :=
endif

all: $(TARGET_BIN) $(TARGET_TEXT) $(TARGET_RODATA) $(TARGET_DATA)
.PHONY += all

clean:
	rm -rf build
.PHONY += clean

bin: $(TARGET_BIN)
.PHONY += bin

verbose:
.PHONY += verbose

build/%.o: src/%.cpp $(THIS_FILE)
	@echo "\x1b[1;34mCompiling $< -> $@\x1b[0m"
	@mkdir -p $(dir $@)
	$(SILENT)$(CXX) -c -o $@ $(CXX_FLAGS) $<

$(TARGET): $(OBJECTS) $(LINKER_SCRIPT) $(THIS_FILE)
	@echo "\x1b[1;34mLinking $@\x1b[0m"
	@mkdir -p $(dir $@)
	$(SILENT)$(LD) -o $@ $(LD_FLAGS) -Map $(TARGET).map $(OBJECTS)

$(TARGET_BIN): $(TARGET)
	@echo "\x1b[1;34mGenerating $@\x1b[0m"
	@mkdir -p $(dir $@)
	$(SILENT)$(OBJCOPY) -O binary $< $@

$(TARGET_TEXT): $(TARGET)
	@echo "\x1b[1;34mGenerating $@\x1b[0m"
	@mkdir -p $(dir $@)
	$(SILENT)$(OBJCOPY) -O binary -j .text $< $@

$(TARGET_RODATA): $(TARGET)
	@echo "\x1b[1;34mGenerating $@\x1b[0m"
	@mkdir -p $(dir $@)
	$(SILENT)$(OBJCOPY) -O binary -j .rodata $< $@

$(TARGET_DATA): $(TARGET)
	@echo "\x1b[1;34mGenerating $@\x1b[0m"
	@mkdir -p $(dir $@)
	$(SILENT)$(OBJCOPY) -O binary -j .data $< $@
