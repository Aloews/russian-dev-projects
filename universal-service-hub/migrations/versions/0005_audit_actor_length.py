"""Widen audit actor identifiers for project-scoped tokens.

Revision ID: 0005_audit_actor_length
Revises: 0004_project_tenancy
Create Date: 2026-11-13
"""

from __future__ import annotations

import sqlalchemy as sa
from alembic import op

revision = "0005_audit_actor_length"
down_revision = "0004_project_tenancy"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("audit_events") as batch_op:
        batch_op.alter_column(
            "actor",
            existing_type=sa.String(length=120),
            type_=sa.String(length=200),
            existing_nullable=False,
        )


def downgrade() -> None:
    with op.batch_alter_table("audit_events") as batch_op:
        batch_op.alter_column(
            "actor",
            existing_type=sa.String(length=200),
            type_=sa.String(length=120),
            existing_nullable=False,
        )
