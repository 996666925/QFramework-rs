extends Control

@onready var counter: Node = get_node("../Counter")

func _ready() -> void:
    $Increase.pressed.connect(counter.increase)

func update_count(value: int) -> void:
    $Count.text = "Count: %d" % value
