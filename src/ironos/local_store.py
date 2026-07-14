from __future__ import annotations

import json
import os
import sqlite3
import sys
from datetime import datetime
from pathlib import Path
from typing import Optional

from platformdirs import user_data_dir

APP_NAME = "TsukuyomiOS"
APP_AUTHOR = "bobbycodes"


def data_dir() -> Path:
    return Path(user_data_dir(APP_NAME, APP_AUTHOR))


def db_path() -> Path:
    path = data_dir()
    path.mkdir(parents=True, exist_ok=True)
    return path / "users.db"


def settings_path() -> Path:
    path = data_dir()
    path.mkdir(parents=True, exist_ok=True)
    return path / "settings.json"


def ensure_schema() -> None:
    conn = sqlite3.connect(str(db_path()))
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            role TEXT NOT NULL DEFAULT 'user',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
        """
    )
    conn.commit()
    conn.close()


def _hash(password: str) -> str:
    import hashlib

    salt = "tsukuyomi"
    return hashlib.sha256((password + salt).encode()).hexdigest()


def authenticate(username: str, password: str) -> Optional[dict]:
    ensure_schema()
    conn = sqlite3.connect(str(db_path()))
    conn.row_factory = sqlite3.Row
    row = conn.execute("SELECT * FROM users WHERE username = ?", (username,)).fetchone()
    conn.close()
    if row and row["password_hash"] == _hash(password):
        return {
            "id": row["id"],
            "username": row["username"],
            "display_name": row["display_name"],
            "role": row["role"],
        }
    return None


def create_user(username: str, password: str, display_name: str = "", role: str = "user") -> bool:
    ensure_schema()
    try:
        conn = sqlite3.connect(str(db_path()))
        conn.execute(
            "INSERT INTO users (username, password_hash, display_name, role) VALUES (?, ?, ?, ?)",
            (username, _hash(password), display_name or username, role),
        )
        conn.commit()
        conn.close()
        return True
    except sqlite3.IntegrityError:
        return False


def list_users() -> list[dict]:
    ensure_schema()
    conn = sqlite3.connect(str(db_path()))
    conn.row_factory = sqlite3.Row
    rows = conn.execute("SELECT id, username, display_name, role, created_at FROM users ORDER BY id").fetchall()
    conn.close()
    return [dict(r) for r in rows]


def delete_all_users() -> None:
    ensure_schema()
    conn = sqlite3.connect(str(db_path()))
    conn.execute("DELETE FROM users")
    conn.commit()
    conn.close()


def load_settings() -> dict:
    path = settings_path()
    defaults = {
        "theme": "dark",
        "timezone": "America/Chicago",
        "language": "en",
        "region": "US",
        "date_format": "%Y-%m-%d",
        "time_format": "%H:%M:%S",
        "use_24h": True,
        "notifications": True,
        "onboarded": False,
    }
    if path.exists():
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
            defaults.update(data)
        except Exception:
            pass
    return defaults


def save_settings(settings: dict) -> None:
    settings_path().write_text(json.dumps(settings, indent=2), encoding="utf-8")
