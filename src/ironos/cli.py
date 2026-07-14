from __future__ import annotations

import click
from ironos.db import create_user, ensure_default_admin, init_db


@click.group()
def cli() -> None:
    pass


@cli.command()
def init() -> None:
    init_db()
    ensure_default_admin()
    print("Tsukuyomi OS database initialized. Default admin: admin / changeme")


@cli.command()
@click.option("--username", required=True)
@click.option("--password", required=True)
@click.option("--display-name", default="")
@click.option("--role", default="user")
def add_user(username: str, password: str, display_name: str, role: str) -> None:
    init_db()
    user_id = create_user(username, password, display_name, role)
    print(f"Created user {username} (id={user_id}).")


if __name__ == "__main__":
    cli()
