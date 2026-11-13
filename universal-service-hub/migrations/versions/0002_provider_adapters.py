"""Add provider adapter metadata.

Revision ID: 0002_provider_adapters
Revises: 0001_provider_registry
Create Date: 2026-11-13
"""

from __future__ import annotations

import sqlalchemy as sa
from alembic import op

revision = "0002_provider_adapters"
down_revision = "0001_provider_registry"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column(
        "providers",
        sa.Column("adapter_type", sa.String(length=32), nullable=False, server_default="none"),
    )
    op.add_column("providers", sa.Column("model", sa.String(length=160), nullable=True))
    op.add_column("providers", sa.Column("base_url", sa.Text(), nullable=True))
    op.add_column("providers", sa.Column("api_key_env", sa.String(length=120), nullable=True))
    op.add_column(
        "providers",
        sa.Column(
            "supports_idempotency",
            sa.Boolean(),
            nullable=False,
            server_default=sa.false(),
        ),
    )


def downgrade() -> None:
    op.drop_column("providers", "supports_idempotency")
    op.drop_column("providers", "api_key_env")
    op.drop_column("providers", "base_url")
    op.drop_column("providers", "model")
    op.drop_column("providers", "adapter_type")
