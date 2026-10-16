"""Waveshare Pico-LCD-1.3 button tester for MicroPython.

Hardware reference and ST7789 initialization settings:
https://github.com/waveshare/Pico_code/blob/main/Python/Pico-LCD-1.3/Pico-LCD-1.3.py
"""
from machine import Pin, SPI, PWM
import framebuf
import time
import _thread


def rgb(r, g, b):
    value = ((r & 248) << 8) | ((g & 252) << 3) | (b >> 3)
    return ((value & 255) << 8) | (value >> 8)


BG = rgb(8, 12, 20)
WHITE = rgb(240, 244, 250)
DIM = rgb(140, 156, 176)
GREEN = rgb(65, 230, 140)
BLUE = rgb(45, 90, 150)


from waveshare_lcd_driver import LCD_1inch3


class Display(LCD_1inch3):
    def __init__(self):
        self.backlight = PWM(Pin(13))
        self.backlight.freq(1000)
        self.backlight.duty_u16(32768)
        super().__init__()
        self.fill(BG)
        self.show()

    def show_rows(self, y, height):
        self.show()


class Button:
    def __init__(self, name, gpio):
        self.name = name
        self.pin = Pin(gpio, Pin.IN, Pin.PULL_UP)
        # Start released so a button held during startup still registers.
        self.raw = 1
        self.stable = 1
        self.changed_at = time.ticks_ms()
        self.started_at = self.changed_at
        self.count = 0
        self.duration = 0
        self.tested = False

    def poll(self, now):
        value = self.pin.value()
        if value != self.raw:
            self.raw = value
            self.changed_at = now
        if value != self.stable and time.ticks_diff(now, self.changed_at) >= 20:
            self.stable = value
            if value == 0:
                self.count += 1
                self.started_at = now
                return self.name + ' DOWN'
            self.duration = time.ticks_diff(now, self.started_at)
            self.tested = True
            return self.name + ' UP'
        return None

    def held_ms(self, now):
        return time.ticks_diff(now, self.started_at) if self.stable == 0 else self.duration


def run():
    lcd = Display()
    buttons = [Button(name, pin) for name, pin in (
        ('A', 15), ('B', 17), ('X', 19), ('Y', 21),
        ('UP', 2), ('DOWN', 18), ('LEFT', 16), ('RIGHT', 20), ('CENTRE', 3),
    )]
    lcd.text('PICO LCD BUTTON TEST', 8, 5, WHITE)
    lcd.text('KEY     STATE  COUNT HOLD', 8, 29, DIM)
    lcd.text('Press + release all 9 inputs', 8, 226, DIM)
    lcd.show_rows(0, 240)
    # Keep sampling on core 1 while core 0 sends a full frame over SPI.
    # Protect snapshots with a short lock; never hold it during a transfer.
    lock = _thread.allocate_lock()
    running = [True]
    last_event = ['Ready - press any button']

    def sample_buttons():
        while running[0]:
            now = time.ticks_ms()
            for button in buttons:
                with lock:
                    event = button.poll(now)
                    if event:
                        last_event[0] = event
                        count = button.count
                        duration = button.held_ms(now)
                if event:
                    print('%s count=%d hold_ms=%d' % (event, count, duration))
            time.sleep_ms(2)

    _thread.start_new_thread(sample_buttons, ())
    print('BUTTON_TEST_READY: nine inputs, 20ms debounce; centre = joystick click')
    try:
        while True:
            with lock:
                now = time.ticks_ms()
                snapshot = [(b.name, b.stable, b.count,
                             min(b.held_ms(now) // 100, 9999), b.tested)
                            for b in buttons]
                event_text = last_event[0]
            for i, (name, stable, count, duration, tested) in enumerate(snapshot):
                y = 45 + i * 16
                pressed = stable == 0
                lcd.fill_rect(0, y, 240, 16, BLUE if pressed else BG)
                color = WHITE if pressed else (GREEN if tested else DIM)
                lcd.text(name, 8, y + 4, color)
                lcd.text('DOWN' if pressed else ' -- ', 72, y + 4, color)
                lcd.text('%4d' % min(count, 9999), 120, y + 4, color)
                lcd.text('%3d.%ds' % (duration // 10, duration % 10), 168, y + 4, color)
            tested = sum(1 for row in snapshot if row[4])
            lcd.fill_rect(0, 193, 240, 30, BG)
            lcd.text(event_text, 8, 195, WHITE)
            lcd.text('Tested: %d/9' % tested, 8, 211, GREEN)
            lcd.text('*' if (now // 500) % 2 else '.', 224, 211, WHITE)
            lcd.show_rows(0, 240)
            time.sleep_ms(10)
    finally:
        running[0] = False
        time.sleep_ms(50)


if __name__ == '__main__':
    run()
