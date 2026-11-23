(
    version: 1,
    entities: [
        (
            id: "019fe400-0000-7000-8000-0000000000a1",
            components: {
                "Name": "Camera",
                "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)),
                "Camera": (size:5.0,priority:0,active:true),
            },
        ),
        (
            id: "019fe400-0000-7000-8000-0000000000a2",
            components: {
                "Name": "Player",
                "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)),
                "Sprite": (color:(0.35,0.75,1.0,1.0),sort_order:1,texture:Some(Path("sprites/checker.png"))),
                "Player": (speed:3.0),
            },
        ),
        (
            id: "019fe400-0000-7000-8000-0000000000a3",
            components: {
                "Name": "Title",
                "Transform": (translation:(0.0,2.5),rotation:0.0,scale:(1.0,1.0)),
                "Text": (content:"{{title}}",size:0.5,pixels_per_em:48,color:(1.0,1.0,1.0,1.0),align:Center,sort_order:2),
            },
        ),
    ],
)
