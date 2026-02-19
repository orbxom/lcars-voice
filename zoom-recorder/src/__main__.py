"""Entry point for zoom-recorder."""

from src.gui import ZoomRecorderApp


def main():
    try:
        app = ZoomRecorderApp()
        app.run()
    except RuntimeError as e:
        print(f"Error: {e}")
        exit(1)


if __name__ == "__main__":
    main()
