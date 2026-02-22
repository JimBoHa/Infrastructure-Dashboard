from __future__ import annotations


def test_ads1115_aliases_to_analog() -> None:
    from app.services.publisher import normalize_sensor_type

    assert normalize_sensor_type("ads1115") == "analog"
    assert normalize_sensor_type("ADS1115") == "analog"

