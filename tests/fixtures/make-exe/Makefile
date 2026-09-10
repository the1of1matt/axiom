TARGET=/tmp/axiom-make-hello
all: $(TARGET)
$(TARGET): main.c
	$(CC) -o $(TARGET) main.c
run: $(TARGET)
	$(TARGET)
clean:
	rm -f $(TARGET)
