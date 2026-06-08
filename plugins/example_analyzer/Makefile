# Example Plugin: Counter Analyzer

CC ?= gcc
CFLAGS = -shared -fPIC -O2 -Wall

all: counter_analyzer.so

counter_analyzer.so: example_analyzer.c
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f counter_analyzer.so

.PHONY: all clean
