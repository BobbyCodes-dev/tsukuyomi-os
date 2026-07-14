from __future__ import annotations

import hashlib
import secrets
import sqlite3
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional

from pydantic import BaseModel, Field


DB_PATH = Path(__file__).with_name("ironos.db")


class User(BaseModel):
    id: int
    username: str
    display_name: str
    role: str = "user"
    theme: str = "dark"
    created_at: datetime


class Session(BaseModel):
    token: str
    user_id: int
    username: str
    expires_at: datetime


class Settings(BaseModel):
    user_id: int
    theme: str = "dark"
    timezone: str = "America/Chicago"
    language: str = "en"
    notifications_enabled: bool = True


@contextmanager
def get_db():
    conn = sqlite3.connect(DB_PATH, check_same_thread=False)
    conn.row_factory = sqlite3.Row
    try:
        yield conn
    finally:
        conn.close()


def init_db() -> None:
    DB_PATH.parent.mkdir(parents=True, exist_ok=True)
    with get_db() as conn:
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

            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id INTEGER NOT NULL,
                expires_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
            """
        )
        conn.commit()


def _hash_password(password: str, salt: Optional[str] = None) -> str:
    if salt is None:
        salt = secrets.token_hex(16)
    pw_hash = hashlib.pbkdf2_hmac("sha256", password.encode(), salt.encode(), 200_000).hex()
    return f"{salt}${pw_hash}"


def _verify_password(password: str, stored: str) -> bool:
    if "$" not in stored:
        return False
    salt, _ = stored.split("$", 1)
    return secrets.compare_digest(stored, _hash_password(password, salt))


def create_user(username: str, password: str, display_name: str = "", role: str = "user") -> int:
    with get_db() as conn:
        cur = conn.execute(
            "INSERT INTO users (username, password_hash, display_name, role) VALUES (?, ?, ?, ?)",
            (username, _hash_password(password), display_name or username, role),
        )
        user_id = cur.lastrowid
        assert user_id is not None
        conn.commit()
        return user_id


def authenticate(username: str, password: str) -> Optional[User]:
    with get_db() as conn:
        row = conn.execute("SELECT * FROM users WHERE username = ?", (username,)).fetchone()
        if not row or not _verify_password(password, row["password_hash"]):
            return None
        return User(
            id=row["id"],
            username=row["username"],
            display_name=row["display_name"],
            role=row["role"],
            created_at=datetime.fromisoformat(row["created_at"]),
        )


def create_session(user_id: int) -> Session:
    token = secrets.token_urlsafe(32)
    expires = datetime.now(timezone.utc) + timedelta(days=1)
    with get_db() as conn:
        conn.execute(
            "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?)",
            (token, user_id, expires.isoformat()),
        )
        conn.commit()
        row = conn.execute("SELECT username FROM users WHERE id = ?", (user_id,)).fetchone()
    return Session(token=token, user_id=user_id, username=row["username"], expires_at=expires)


def get_session(token: str) -> Optional[Session]:
    with get_db() as conn:
        conn.execute("DELETE FROM sessions WHERE expires_at < ?", (datetime.now(timezone.utc).isoformat(),))
        row = conn.execute(
            "SELECT s.token, s.user_id, u.username, s.expires_at "
            "FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token = ?",
            (token,),
        ).fetchone()
        if not row:
            return None
        return Session(
            token=row["token"],
            user_id=row["user_id"],
            username=row["username"],
            expires_at=datetime.fromisoformat(row["expires_at"]),
        )


def delete_session(token: str) -> None:
    with get_db() as conn:
        conn.execute("DELETE FROM sessions WHERE token = ?", (token,))
        conn.commit()


def get_user(user_id: int) -> Optional[User]:
    with get_db() as conn:
        row = conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)).fetchone()
        if not row:
            return None
        return User(
            id=row["id"],
            username=row["username"],
            display_name=row["display_name"],
            role=row["role"],
            created_at=datetime.fromisoformat(row["created_at"]),
        )


def list_users() -> list[User]:
    with get_db() as conn:
        rows = conn.execute("SELECT * FROM users ORDER BY id").fetchall()
        return [
            User(
                id=r["id"],
                username=r["username"],
                display_name=r["display_name"],
                role=r["role"],
                created_at=datetime.fromisoformat(r["created_at"]),
            )
            for r in rows
        ]


def ensure_default_admin() -> None:
    with get_db() as conn:
        row = conn.execute("SELECT id FROM users WHERE role = 'admin' LIMIT 1").fetchone()
        if row:
            return
    create_user("admin", "changeme", display_name="Administrator", role="admin")
