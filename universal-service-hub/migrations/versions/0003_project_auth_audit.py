"""Add project-scoped keys and audit events.

Revision ID: 0003_project_auth_audit
Revises: 0002_provider_adapters
Create Date: 2026-11-13
"""

from __future__ import annotations

import sqlalchemy as sa
from alembic import op

revision = "0003_project_auth_audit"
down_revision = "0002_provider_adapters"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.create_table(
        "projects",
        sa.Column("id", sa.String(length=80), primary_key=True),
        sa.Column("name", sa.String(length=160), nullable=False),
        sa.Column("enabled", sa.Boolean(), nullable=False, server_default=sa.true()),
        sa.Column("created_at", sa.DateTime(timezone=True), nullable=False),
    )
    with op.batch_alter_table("providers") as batch_op:
        batch_op.add_column(sa.Column("project_id", sa.String(length=80), nullable=True))
        batch_op.create_foreign_key(
            "fk_providers_project_id_projects",
            "projects",
            ["project_id"],
            ["id"],
            ondelete="SET NULL",
        )
        batch_op.create_index(
            "ix_providers_project_enabled_priority",
            ["project_id", "enabled", "priority"],
        )
    op.create_table(
        "api_keys",
        sa.Column("id", sa.String(length=36), primary_key=True),
        sa.Column("name", sa.String(length=120), nullable=False),
        sa.Column("token_hash", sa.String(length=64), nullable=False),
        sa.Column("token_prefix", sa.String(length=24), nullable=False),
        sa.Column("role", sa.String(length=20), nullable=False),
        sa.Column("project_id", sa.String(length=80), nullable=True),
        sa.Column("scopes", sa.JSON(), nullable=False),
        sa.Column("enabled", sa.Boolean(), nullable=False, server_default=sa.true()),
        sa.Column("created_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("expires_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("last_used_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("revoked_at", sa.DateTime(timezone=True), nullable=True),
        sa.ForeignKeyConstraint(["project_id"], ["projects.id"], ondelete="CASCADE"),
    )
    op.create_index("ix_api_keys_token_hash", "api_keys", ["token_hash"], unique=True)
    op.create_table(
        "audit_logs",
        sa.Column("id", sa.String(length=36), primary_key=True),
        sa.Column("actor_id", sa.String(length=80), nullable=False),
        sa.Column("project_id", sa.String(length=80), nullable=True),
        sa.Column("action", sa.String(length=100), nullable=False),
        sa.Column("resource_type", sa.String(length=80), nullable=False),
        sa.Column("resource_id", sa.String(length=120), nullable=True),
        sa.Column("outcome", sa.String(length=20), nullable=False),
        sa.Column("correlation_id", sa.String(length=80), nullable=False),
        sa.Column("metadata", sa.JSON(), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), nullable=False),
        sa.ForeignKeyConstraint(["project_id"], ["projects.id"], ondelete="SET NULL"),
    )
    op.create_index("ix_audit_logs_project_created", "audit_logs", ["project_id", "created_at"])
    op.create_index("ix_audit_logs_actor_created", "audit_logs", ["actor_id", "created_at"])


def downgrade() -> None:
    op.drop_index("ix_audit_logs_actor_created", table_name="audit_logs")
    op.drop_index("ix_audit_logs_project_created", table_name="audit_logs")
    op.drop_table("audit_logs")
    op.drop_index("ix_api_keys_token_hash", table_name="api_keys")
    op.drop_table("api_keys")
    with op.batch_alter_table("providers") as batch_op:
        batch_op.drop_index("ix_providers_project_enabled_priority")
        batch_op.drop_constraint("fk_providers_project_id_projects", type_="foreignkey")
        batch_op.drop_column("project_id")
    op.drop_table("projects")
