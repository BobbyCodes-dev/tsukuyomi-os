from __future__ import annotations

import datetime
import hashlib
import random
import secrets
import sqlite3
import string
import time
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Optional

from fastapi import APIRouter, Cookie, Depends, FastAPI, Form, HTTPException, Request, Response, UploadFile
from fastapi.responses import FileResponse, HTMLResponse, JSONResponse, RedirectResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from pydantic import BaseModel

from ironos.db import (
    Session,
    User,
    authenticate,
    create_session,
    create_user,
    delete_session,
    ensure_default_admin,
    get_session,
    get_user,
    init_db,
    list_users,
)

BASE_DIR = Path(__file__).resolve().parent.parent.parent
TEMPLATES_DIR = BASE_DIR / "templates"
STATIC_DIR = BASE_DIR / "static"

router = APIRouter()
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))


class AuthContext(BaseModel):
    user: User
    session: Session


async def require_auth(request: Request) -> AuthContext:
    token = request.cookies.get("session")
    if not token:
        raise HTTPException(status_code=401, detail="Not authenticated")
    session = get_session(token)
    if not session:
        raise HTTPException(status_code=401, detail="Session expired")
    user = get_user(session.user_id)
    if not user:
        raise HTTPException(status_code=401, detail="User not found")
    return AuthContext(user=user, session=session)


@asynccontextmanager
async def lifespan(app: FastAPI):
    init_db()
    ensure_default_admin()
    yield


def create_app() -> FastAPI:
    app = FastAPI(title="IronOS", lifespan=lifespan)
    app.include_router(router)
    if STATIC_DIR.exists():
        app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")
    return app


@router.get("/", response_class=HTMLResponse)
async def root(request: Request):
    token = request.cookies.get("session")
    if token and get_session(token):
        return RedirectResponse(url="/desktop")
    return templates.TemplateResponse(request=request, name="login.html", context={"error": None})


@router.get("/login", response_class=HTMLResponse)
async def login_page(request: Request, error: Optional[str] = None):
    return templates.TemplateResponse(request=request, name="login.html", context={"error": error})


@router.post("/login")
async def login_post(response: Response, username: str = Form(...), password: str = Form(...)):
    user = authenticate(username, password)
    if not user:
        return RedirectResponse(url="/login?error=invalid", status_code=303)
    session = create_session(user.id)
    resp = RedirectResponse(url="/desktop", status_code=303)
    resp.set_cookie(
        key="session",
        value=session.token,
        httponly=True,
        secure=False,
        samesite="lax",
        max_age=86400,
    )
    return resp


@router.post("/logout")
async def logout_post(request: Request, response: Response):
    token = request.cookies.get("session")
    if token:
        delete_session(token)
    response.delete_cookie("session")
    return RedirectResponse(url="/", status_code=303)


@router.get("/desktop", response_class=HTMLResponse)
async def desktop(request: Request, auth: AuthContext = Depends(require_auth)):
    return templates.TemplateResponse(
        request=request,
        name="desktop.html",
        context={
            "username": auth.user.username,
            "display_name": auth.user.display_name,
        },
    )


@router.get("/api/me")
async def api_me(auth: AuthContext = Depends(require_auth)):
    return {
        "user": {
            "id": auth.user.id,
            "username": auth.user.username,
            "display_name": auth.user.display_name,
            "role": auth.user.role,
        },
    }


@router.get("/app/{app_id}", response_class=HTMLResponse)
async def app_page(request: Request, app_id: str, auth: AuthContext = Depends(require_auth)):
    if app_id == "anyrun":
        return templates.TemplateResponse(request=request, name="anyrun.html", context={"username": auth.user.username})
    raise HTTPException(status_code=404, detail="App not found")


@router.get("/api/apps")
async def api_apps(auth: AuthContext = Depends(require_auth)):
    return {
        "apps": [
            {
                "id": "anyrun",
                "name": "Tsukuyomi Sandbox",
                "description": "Interactive malware analysis sandbox TUI clone.",
                "icon": "🧪",
                "action": "iframe",
                "url": "/app/anyrun",
            },
            {
                "id": "browser",
                "name": "Tsukuyomi Browser",
                "description": "Embedded browser.",
                "icon": "🌐",
                "action": "iframe",
                "url": "https://duckduckgo.com",
            },
            {
                "id": "terminal",
                "name": "Terminal",
                "description": "Web-based terminal.",
                "icon": "💻",
                "action": "terminal",
                "url": None,
            },
            {
                "id": "files",
                "name": "Tsukuyomi Files",
                "description": "File manager.",
                "icon": "📁",
                "action": "placeholder",
                "url": None,
            },
            {
                "id": "settings",
                "name": "Settings",
                "description": "User and system settings.",
                "icon": "⚙️",
                "action": "settings",
                "url": None,
            },
        ]
    }


@router.post("/api/admin/users")
async def api_create_user(
    username: str = Form(...),
    password: str = Form(...),
    display_name: str = Form(""),
    role: str = Form("user"),
    auth: AuthContext = Depends(require_auth),
):
    if auth.user.role != "admin":
        raise HTTPException(status_code=403, detail="Admins only")
    try:
        user_id = create_user(username, password, display_name, role)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    return {"id": user_id, "username": username}


@router.get("/api/admin/users")
async def api_list_users(auth: AuthContext = Depends(require_auth)):
    if auth.user.role != "admin":
        raise HTTPException(status_code=403, detail="Admins only")
    return {"users": [u.model_dump(exclude={"created_at"}) for u in list_users()]}


# --- Sandbox simulation ---

SANDBOX_TASKS: dict[str, dict] = {}


def _risk_score(target: str) -> int:
    seed = sum(ord(c) for c in target)
    return min(95, max(10, (seed % 80) + random.randint(0, 15)))


def _fake_sha() -> str:
    return "".join(random.choices("0123456789abcdef", k=64))


def _build_report(target: str, os_profile: str) -> dict:
    score = _risk_score(target)
    domains = ["cdn-update.com", "api-metrics.io", "ssl-telemetry.net", "windows-update.live"]
    dropped = [
        {"path": f"C:\\Users\\Admin\\AppData\\Local\\Temp\\{''.join(random.choices(string.ascii_lowercase, k=8))}.tmp", "action": random.choice(["created", "modified", "deleted"]), "sha256": _fake_sha()}
        for _ in range(random.randint(2, 5))
    ]
    network = [
        {"time": datetime.datetime.now().isoformat(), "protocol": random.choice(["TCP/443", "UDP/53", "HTTP/80"]), "destination": f"{random.choice(domains)}:{random.randint(1000, 65535)}", "status": random.choice(["allowed", "blocked", "dns-only"])}
        for _ in range(random.randint(3, 8))
    ]
    iocs = [
        {"type": "domain", "value": random.choice(domains), "severity": random.choice(["low", "medium", "high"])},
        {"type": "ip", "value": f"{random.randint(1,255)}.{random.randint(0,255)}.{random.randint(0,255)}.{random.randint(0,255)}", "severity": random.choice(["low", "medium", "high"])},
        {"type": "mutex", "value": f"Global\\{''.join(random.choices(string.ascii_uppercase, k=12))}", "severity": random.choice(["low", "medium", "high"])},
    ]
    registry = [
        {"hive": "HKCU", "key": "Software\\Microsoft\\Windows\\CurrentVersion\\Run", "value": target},
        {"hive": "HKLM", "key": "SYSTEM\\CurrentControlSet\\Services", "value": "sandbox_driver"},
    ]
    return {
        "target_name": target,
        "os_profile": os_profile,
        "risk_score": score,
        "dropped_files": dropped,
        "network_events": network,
        "iocs": iocs,
        "registry": registry,
    }


def _log_lines(target: str, os_profile: str) -> list[dict]:
    base = [
        f"Allocating {os_profile} VM...",
        "Mounting sample volume",
        "Starting instrumentation agents",
        f"Detonating target: {target}",
    ]
    events = [
        "Process created: sample.exe (PID 4824)",
        "Process created: cmd.exe /c whoami (PID 4912)",
        "Registry write: HKCU\\...\\Run",
        "File drop: AppData\\Local\\Temp\\*.tmp",
        "DNS query: cdn-update.com",
        "TCP connect: 185.220.101.44:443",
        "HTTP POST /api/v2/beacon (encrypted)",
        "Scheduled task created",
        "Injecting explorer.exe attempted",
        "VM shutdown requested",
    ]
    return [{"time": datetime.datetime.now(datetime.timezone.utc).isoformat(), "message": m} for m in base + random.sample(events, min(len(events), random.randint(4, 8)))]


@router.post("/api/sandbox/submit")
async def sandbox_submit(
    type: str = Form(...),
    target: str = Form(...),
    os_profile: str = Form("win10"),
    script: Optional[str] = Form(None),
    file: Optional[UploadFile] = None,
    auth: AuthContext = Depends(require_auth),
):
    task_id = "TASK-" + secrets.token_hex(6).upper()
    SANDBOX_TASKS[task_id] = {
        "id": task_id,
        "status": "RUNNING",
        "target_name": target,
        "os_profile": os_profile,
        "submitted_at": time.time(),
        "logs": _log_lines(target, os_profile),
        "report": None,
    }
    return {"id": task_id}


@router.get("/api/sandbox/{task_id}/status")
async def sandbox_status(task_id: str, auth: AuthContext = Depends(require_auth)):
    task = SANDBOX_TASKS.get(task_id)
    if not task:
        raise HTTPException(status_code=404, detail="Task not found")
    elapsed = time.time() - task["submitted_at"]
    if task["status"] == "RUNNING" and elapsed > 3:
        task["status"] = "DONE"
        task["report"] = _build_report(task["target_name"], task["os_profile"])
    return task


@router.post("/api/terminal")
async def api_terminal(cmd: dict, auth: AuthContext = Depends(require_auth)):
    command = cmd.get("cmd", "").strip()
    if not command:
        return {"output": ""}
    safe = {"help", "whoami", "date", "uname", "ls", "pwd", "uptime", "hostname", "df", "free", "ps"}
    base = command.split()[0].lower()
    if base not in safe:
        return {"error": f"Command not allowed: {base}. Allowed: {', '.join(sorted(safe))}"}
    try:
        import subprocess
        result = subprocess.run(command.split(), capture_output=True, text=True, timeout=5)
        return {"output": (result.stdout + result.stderr)[:2000]}
    except Exception as e:
        return {"error": str(e)}


@router.get("/favicon.ico")
async def favicon():
    return Response(status_code=204)
