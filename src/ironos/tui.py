from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
from pathlib import Path

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.screen import Screen
from textual.widgets import (
    Button,
    DataTable,
    Footer,
    Header,
    Input,
    Label,
    Log,
    Markdown,
    Static,
    Select,
)

from ironos.local_store import (
    authenticate,
    create_user,
    data_dir,
    delete_all_users,
    list_users,
    load_settings,
    save_settings,
)
from ironos.vm_manager import choose_backend, detect_backends, launch_vm, suggest_action

APPS = [
    {
        "id": "sandbox",
        "name": "Tsukuyomi Sandbox",
        "description": "Launch an isolated Windows VM for malware analysis.",
        "icon": "🧪",
        "category": "Security",
    },
    {
        "id": "browser",
        "name": "Tsukuyomi Browser",
        "description": "Open an embedded browser.",
        "icon": "🌐",
        "category": "Productivity",
    },
    {
        "id": "terminal",
        "name": "Terminal",
        "description": "Local system shell.",
        "icon": "💻",
        "category": "System",
    },
    {
        "id": "files",
        "name": "Tsukuyomi Files",
        "description": "File manager.",
        "icon": "📁",
        "category": "System",
    },
    {
        "id": "settings",
        "name": "Settings",
        "description": "Configure Tsukuyomi OS.",
        "icon": "⚙️",
        "category": "System",
    },
]


def now_string() -> str:
    import datetime

    settings = load_settings()
    tz = settings.get("timezone", "America/Chicago")
    try:
        from zoneinfo import ZoneInfo
        dt = datetime.datetime.now(ZoneInfo(tz))
    except Exception:
        dt = datetime.datetime.now()
    return dt.strftime("%a, %b %d, %Y  %I:%M:%S %p")


class SetupScreen(Screen):
    BINDINGS = [("escape", "quit", "Quit")]

    def compose(self) -> ComposeResult:
        settings = load_settings()
        yield Header(show_clock=True)
        yield Vertical(
            Static("🌙 Tsukuyomi OS Setup", classes="title"),
            Static("Welcome. Create your local account to continue.", classes="subtitle"),
            Static("Username"),
            Input(placeholder="username", id="setup-username"),
            Static("Password"),
            Input(placeholder="password", password=True, id="setup-password"),
            Static("Confirm Password"),
            Input(placeholder="confirm password", password=True, id="setup-password2"),
            Static("Display Name (optional)"),
            Input(placeholder="Display Name", id="setup-display"),
            Static("Timezone"),
            Select(
                [(tz, tz) for tz in self.common_timezones()],
                id="setup-timezone",
                value=settings.get("timezone", "America/Chicago"),
            ),
            Static("Region"),
            Input(placeholder="US", id="setup-region", value=settings.get("region", "US")),
            Static("Language"),
            Input(placeholder="en", id="setup-language", value=settings.get("language", "en")),
            Static("Time Format"),
            Select(
                [("24-hour", "24"), ("12-hour", "12")],
                id="setup-time-format",
                value="24" if settings.get("use_24h", True) else "12",
            ),
            Static("Date Format"),
            Select(
                [
                    ("YYYY-MM-DD", "%Y-%m-%d"),
                    ("MM/DD/YYYY", "%m/%d/%Y"),
                    ("DD/MM/YYYY", "%d/%m/%Y"),
                    ("Mon DD, YYYY", "%b %d, %Y"),
                ],
                id="setup-date-format",
                value=settings.get("date_format", "%Y-%m-%d"),
            ),
            Button("Create Account", id="setup-create", variant="primary"),
            Static("", id="setup-error"),
            classes="login-form",
        )
        yield Footer()

    @staticmethod
    def common_timezones() -> list[str]:
        return [
            "America/New_York",
            "America/Chicago",
            "America/Denver",
            "America/Los_Angeles",
            "Europe/London",
            "Europe/Paris",
            "Europe/Berlin",
            "Asia/Tokyo",
            "Asia/Shanghai",
            "Australia/Sydney",
            "UTC",
        ]

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id != "setup-create":
            return
        username = self.query_one("#setup-username", Input).value.strip()
        password = self.query_one("#setup-password", Input).value
        password2 = self.query_one("#setup-password2", Input).value
        display = self.query_one("#setup-display", Input).value.strip() or username
        timezone = self.query_one("#setup-timezone", Select).value or "America/Chicago"
        region = self.query_one("#setup-region", Input).value.strip() or "US"
        language = self.query_one("#setup-language", Input).value.strip() or "en"
        time_format = self.query_one("#setup-time-format", Select).value or "24"
        date_format = self.query_one("#setup-date-format", Select).value or "%Y-%m-%d"
        error = self.query_one("#setup-error", Static)

        if not username or not password:
            error.update("Username and password are required.")
            return
        if password != password2:
            error.update("Passwords do not match.")
            return
        if len(password) < 6:
            error.update("Password must be at least 6 characters.")
            return

        if not create_user(username, password, display_name=display, role="admin"):
            error.update("Username already exists.")
            return

        settings = load_settings()
        settings.update({
            "timezone": str(timezone),
            "region": region,
            "language": language,
            "use_24h": time_format == "24",
            "time_format": "%H:%M:%S" if time_format == "24" else "%I:%M:%S %p",
            "date_format": str(date_format),
            "onboarded": True,
        })
        save_settings(settings)
        self.app.user = authenticate(username, password)
        self.app.pop_screen()
        self.app.push_screen(DesktopScreen())


class LoginScreen(Screen):
    BINDINGS = [("q", "quit", "Quit"), ("r", "reset_setup", "Reset Setup")]

    def action_reset_setup(self) -> None:
        self.app.push_screen(SetupScreen())

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        yield Vertical(
            Static("🌙 Tsukuyomi OS", classes="title"),
            Static("Terminal-based personal OS shell", classes="subtitle"),
            Input(placeholder="Username", id="username"),
            Input(placeholder="Password", password=True, id="password"),
            Button("Sign In", id="login", variant="primary"),
            Static("", id="error"),
            classes="login-form",
        )
        yield Footer()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id != "login":
            return
        username = self.query_one("#username", Input).value
        password = self.query_one("#password", Input).value
        user = authenticate(username, password)
        if user:
            self.app.user = user
            self.app.push_screen(DesktopScreen())
        else:
            self.query_one("#error", Static).update("Invalid username or password.")


class DesktopScreen(Screen):
    BINDINGS = [
        ("q", "quit", "Quit"),
        ("r", "refresh", "Refresh"),
        ("s", "settings", "Settings"),
    ]

    def compose(self) -> ComposeResult:
        user = getattr(self.app, "user", {"username": "guest", "display_name": "Guest", "role": "user"})
        yield Header(show_clock=True)
        yield Horizontal(
            Vertical(
                Static(f"🌙 Tsukuyomi OS", classes="os-title"),
                Static(f"User: {user.get('display_name')} ({user.get('username')})  |  Role: {user.get('role')}", classes="user-info"),
                Static(now_string(), id="clock", classes="clock"),
                Static("Use ↑/↓ to navigate, Enter to launch, 's' for settings, 'q' to quit.", classes="hint"),
                DataTable(id="app-grid"),
                Log(id="status-log", highlight=True, wrap=True),
                classes="desktop",
            ),
            classes="desktop-container",
        )
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#app-grid", DataTable)
        table.cursor_type = "row"
        table.add_columns("Icon", "App", "Description", "Category")
        for app in APPS:
            table.add_row(app["icon"], app["name"], app["description"], app["category"])
        self.update_clock()
        self.set_interval(1, self.update_clock)
        self.log_status("Welcome to Tsukuyomi OS. Select an app and press Enter.")

    def update_clock(self) -> None:
        self.query_one("#clock", Static).update(now_string())

    def log_status(self, message: str) -> None:
        log = self.query_one("#status-log", Log)
        log.write_line(f"[{now_string()}] {message}")

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        table = self.query_one("#app-grid", DataTable)
        row_key = event.row_key.value
        app = APPS[int(row_key)]
        self.launch_app(app)

    def launch_app(self, app: dict) -> None:
        self.log_status(f"Launching {app['name']}...")
        if app["id"] == "sandbox":
            self.app.push_screen(SandboxScreen())
        elif app["id"] == "browser":
            self.open_browser()
        elif app["id"] == "terminal":
            self.open_terminal()
        elif app["id"] == "files":
            self.open_files()
        elif app["id"] == "settings":
            self.app.push_screen(SettingsScreen())

    def open_browser(self) -> None:
        url = "https://duckduckgo.com"
        if shutil.which("start") and platform.system() == "Windows":
            subprocess.Popen(["start", url], shell=True)
        elif shutil.which("xdg-open"):
            subprocess.Popen(["xdg-open", url])
        elif shutil.which("open"):
            subprocess.Popen(["open", url])
        else:
            self.log_status("No browser launcher found.")

    def open_terminal(self) -> None:
        if platform.system() == "Windows":
            subprocess.Popen(["powershell.exe"])
        else:
            subprocess.Popen([os.environ.get("SHELL", "/bin/bash")])
        self.log_status("Terminal opened externally.")

    def open_files(self) -> None:
        if platform.system() == "Windows":
            subprocess.Popen(["explorer.exe", str(Path.home())])
        elif shutil.which("xdg-open"):
            subprocess.Popen(["xdg-open", str(Path.home())])
        elif shutil.which("open"):
            subprocess.Popen(["open", str(Path.home())])
        self.log_status("File manager opened externally.")

    def action_refresh(self) -> None:
        self.update_clock()
        self.log_status("Refreshed.")

    def action_settings(self) -> None:
        self.app.push_screen(SettingsScreen())


class SandboxScreen(Screen):
    BINDINGS = [("escape", "pop_screen", "Back")]

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        yield Vertical(
            Static("🧪 Tsukuyomi Sandbox", classes="title"),
            Static("Select a VM backend to launch an isolated Windows environment.", classes="subtitle"),
            DataTable(id="backend-list"),
            Log(id="sandbox-log", highlight=True, wrap=True),
            Horizontal(
                Button("Launch", id="launch", variant="primary"),
                Button("Back", id="back"),
            ),
            classes="sandbox-form",
        )
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#backend-list", DataTable)
        table.cursor_type = "row"
        table.add_columns("Backend", "Available", "Notes")
        self.backends = detect_backends()
        self.backend_rows = [b.id for b in self.backends]
        for b in self.backends:
            table.add_row(b.name, "✅ Yes" if b.available else "❌ No", b.reason)
        self.log("Detecting VM backends...")
        chosen = choose_backend(self.backends)
        if chosen:
            self.log(suggest_action(self.backends))
        else:
            self.log(suggest_action(self.backends))

    def log(self, message: str) -> None:
        self.query_one("#sandbox-log", Log).write_line(message)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "back":
            self.app.pop_screen()
        elif event.button.id == "launch":
            table = self.query_one("#backend-list", DataTable)
            if table.cursor_row is None:
                # Auto-launch best backend if nothing selected
                backend_obj = choose_backend(self.backends)
                if not backend_obj:
                    self.log(suggest_action(self.backends))
                    return
                backend = backend_obj.id
            else:
                backend = self.backend_rows[table.cursor_row]
                backend_obj = next((b for b in self.backends if b.id == backend), None)
            if not backend_obj or not backend_obj.available:
                self.log(f"{backend} is not available on this machine.")
                return
            try:
                if backend == "virtualbox":
                    from ironos.vm_builder import build_or_download_vm
                    vdi = build_or_download_vm(data_dir() / "vm")
                    if not vdi.exists():
                        self.log(f"Run 'tsukuyomi-build-vm' to create {vdi}")
                        return
                    try:
                        from ironos.vm_manager import create_virtualbox_vm
                        create_virtualbox_vm("TsukuyomiOS", vdi)
                    except Exception as e:
                        self.log(f"VM setup: {e}")
                launch_vm(backend)
                self.log(f"Launched {backend}.")
            except Exception as e:
                self.log(f"Failed to launch {backend}: {e}")


class SettingsScreen(Screen):
    BINDINGS = [("escape", "pop_screen", "Back")]

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        yield Vertical(
            Static("⚙️ Settings", classes="title"),
            Static("Theme"), Input(id="theme", value="dark"),
            Static("Timezone"), Input(id="timezone", value="America/Chicago"),
            Static("Language"), Input(id="language", value="en"),
            Static("Notifications"), Input(id="notifications", value="true"),
            Horizontal(
                Button("Save", id="save", variant="primary"),
                Button("Back", id="back"),
            ),
            Static("", id="settings-status"),
            classes="settings-form",
        )
        yield Footer()

    def on_mount(self) -> None:
        settings = load_settings()
        self.query_one("#theme", Input).value = settings.get("theme", "dark")
        self.query_one("#timezone", Input).value = settings.get("timezone", "America/Chicago")
        self.query_one("#language", Input).value = settings.get("language", "en")
        self.query_one("#notifications", Input).value = str(settings.get("notifications", True)).lower()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "back":
            self.app.pop_screen()
        elif event.button.id == "save":
            settings = {
                "theme": self.query_one("#theme", Input).value,
                "timezone": self.query_one("#timezone", Input).value,
                "language": self.query_one("#language", Input).value,
                "notifications": self.query_one("#notifications", Input).value.lower() in ("true", "1", "yes", "on"),
            }
            save_settings(settings)
            self.query_one("#settings-status", Static).update("Settings saved locally.")


class TsukuyomiApp(App):
    CSS_PATH = "tsukuyomi.tcss"
    SCREENS = {"login": LoginScreen, "desktop": DesktopScreen, "setup": SetupScreen}
    BINDINGS = [("q", "quit", "Quit")]
    user: dict | None = None

    def on_mount(self) -> None:
        settings = load_settings()
        users = list_users()
        if not settings.get("onboarded") or not users:
            delete_all_users()
            # Clear onboarded flag if no users
            settings["onboarded"] = False
            save_settings(settings)
            self.push_screen(SetupScreen())
        else:
            self.push_screen(LoginScreen())


def main() -> None:
    app = TsukuyomiApp()
    app.run()


if __name__ == "__main__":
    main()
