from __future__ import annotations

from datetime import UTC, datetime

from sqlalchemy import JSON, Boolean, DateTime, Float, Integer, String, Text
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column


class Base(DeclarativeBase):
    pass


class ProviderRecord(Base):
    __tablename__ = "providers"

    id: Mapped[str] = mapped_column(String(80), primary_key=True)
    name: Mapped[str] = mapped_column(String(160), nullable=False)
    capabilities: Mapped[list[str]] = mapped_column(JSON, nullable=False, default=list)
    priority: Mapped[int] = mapped_column(Integer, nullable=False, default=100)
    enabled: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    health_url: Mapped[str | None] = mapped_column(Text, nullable=True)
    adapter_type: Mapped[str] = mapped_column(String(32), nullable=False, default="none")
    model: Mapped[str | None] = mapped_column(String(160), nullable=True)
    base_url: Mapped[str | None] = mapped_column(Text, nullable=True)
    api_key_env: Mapped[str | None] = mapped_column(String(120), nullable=True)
    supports_idempotency: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    timeout_seconds: Mapped[float] = mapped_column(Float, nullable=False, default=3.0)
    health_status: Mapped[str] = mapped_column(String(20), nullable=False, default="unknown")
    checked_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    latency_ms: Mapped[float | None] = mapped_column(Float, nullable=True)
    health_detail: Mapped[str | None] = mapped_column(String(160), nullable=True)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, default=lambda: datetime.now(UTC)
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True),
        nullable=False,
        default=lambda: datetime.now(UTC),
        onupdate=lambda: datetime.now(UTC),
    )
