//! A simplified implementation of the classic game "Breakout".
use bevy_turborand::prelude::*;

use bevy::{
    math::bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume},
    prelude::*,
    sprite::MaterialMesh2dBundle,
};

mod stepping;

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PADDLE_SPEED: f32 = 500.0;

// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
const BALL_DIAMETER: f32 = 30.;
const BALL_SPEED: f32 = 400.0;

// x coordinates
const LEFT_WALL: f32 = -450.;
const RIGHT_WALL: f32 = 450.;

const SCOREBOARD_FONT_SIZE: f32 = 40.0;
const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);

const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);
const PADDLE_COLOR: Color = Color::rgb(0.3, 0.3, 0.7);
const BALL_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
const TEXT_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const SCORE_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // .add_plugins(
        //     stepping::SteppingPlugin::default()
        //         .add_schedule(Update)
        //         .add_schedule(FixedUpdate)
        //         .at(Val::Percent(35.0), Val::Percent(50.0)),
        // )
        .insert_resource(Scoreboard { score: 0 })
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .init_resource::<GlobalRng>()
        .add_event::<CollisionEvent>()
        .add_systems(Startup, setup)
        // Add our gameplay simulation systems to the fixed timestep schedule
        // which runs at 64 Hz by default
        .add_systems(FixedUpdate, despawn_offscreen)
        .add_systems(FixedUpdate, maybe_spawn_meteor)
        .add_systems(
            Update,
            (move_player, apply_velocity, check_for_collisions).chain(),
        )
        .add_systems(Update, (update_scoreboard, bevy::window::close_on_esc))
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Meteor;

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Component)]
struct Collider;

#[derive(Event, Default)]
struct CollisionEvent;

// #[derive(Resource)]
// struct CollisionSound(Handle<AudioSource>);

/// Percentage difficulty, represents chance of meteor spawning in given FixedUpdate tick
#[derive(Resource)]
struct Difficulty(f64);

// This resource tracks the game's score
#[derive(Resource)]
struct Scoreboard {
    score: usize,
}

#[derive(Component)]
struct ScoreboardUi;

fn bottom(w: &Window) -> f32 {
    return w.height() / -2.;
}

fn top(w: &Window) -> f32 {
    return w.height() / 2.;
}

fn left(w: &Window) -> f32 {
    return w.width() / -2.;
}

fn right(w: &Window) -> f32 {
    return w.width() / 2.;
}

// Add the game's entities to our world
fn setup(
    mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
    //mut materials: ResMut<Assets<ColorMaterial>>,
    mut rng: ResMut<GlobalRng>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    window: Query<&Window>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    let player_texture = asset_server.load("Player.png");
    let atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        Vec2::new(100., 100.),
        3,
        3,
        Some(Vec2::new(10., 10.)),
        None,
    ));

    // 140 from left side
    // Start at 1% chance of spawning meteor
    commands.insert_resource(Difficulty(0.1));

    commands.spawn((Player, RngComponent::from(&mut rng)));

    commands.spawn((
        SpriteSheetBundle {
            transform: Transform {
                translation: Vec3::new(0.0, bottom(window.single()), 0.0),
                scale: Vec3 {
                    x: 100.,
                    y: 100.,
                    z: 0.,
                },
                ..default()
            },
            texture: player_texture,
            atlas: TextureAtlas {
                layout: atlas_layout,
                index: 6,
            },
            sprite: Sprite {
                color: PADDLE_COLOR,
                ..default()
            },
            ..default()
        },
        Player,
        Collider,
    ));

    // Scoreboard
    commands.spawn((
        ScoreboardUi,
        TextBundle::from_sections([
            TextSection::new(
                "Score: ",
                TextStyle {
                    font_size: SCOREBOARD_FONT_SIZE,
                    color: TEXT_COLOR,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: SCOREBOARD_FONT_SIZE,
                color: SCORE_COLOR,
                ..default()
            }),
        ])
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: SCOREBOARD_TEXT_PADDING,
            left: SCOREBOARD_TEXT_PADDING,
            ..default()
        }),
    ));
}

fn move_player(
    key_in: Res<ButtonInput<KeyCode>>,
    mut player_q: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
    window: Query<&Window>,
) {
    let mut player_transform = player_q.single_mut();
    let mut direction = 0.0;

    if key_in.pressed(KeyCode::ArrowLeft) {
        direction -= 1.0;
    }

    if key_in.pressed(KeyCode::ArrowRight) {
        direction += 1.0;
    }

    // Calculate the new horizontal paddle position based on player input
    let player_position =
        player_transform.translation.x + direction * PADDLE_SPEED * time.delta_seconds();
    let w = window.single();
    player_transform.translation.x = player_position.clamp(left(w), right(w));
}

// TODO SpawnMeteorEvent
fn maybe_spawn_meteor(
    difficulty: Res<Difficulty>,
    window: Query<&Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut rng: Query<&mut RngComponent, With<Player>>,
) {
    let mut c_rng = rng.single_mut();
    if c_rng.chance(difficulty.0) {
        commands.spawn((
            Meteor,
            Collider,
            Velocity(
                Vec2 {
                    x: c_rng.f32_normalized() / 15.,
                    y: -c_rng.f32(),
                }
                .normalize()
                    * BALL_SPEED,
            ),
            MaterialMesh2dBundle {
                mesh: meshes.add(Circle::default()).into(), // TODO this seems silly, re-use?
                material: materials.add(BALL_COLOR),
                transform: Transform::from_translation(Vec3::new(
                    c_rng.i32((LEFT_WALL as i32)..(RIGHT_WALL as i32)) as f32,
                    window.single().height() / 2.,
                    1.0,
                ))
                .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
                ..default()
            },
        ));
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

fn despawn_offscreen(
    query: Query<(Entity, &Transform), With<Meteor>>,
    mut commands: Commands,
    window: Query<&Window>,
) {
    let max_x = window.single().width() / 2.;
    let min_x = window.single().width() / -2.;
    let min_y = window.single().height() / -2.;
    let max_y = window.single().height() / 2.;
    for (e, xform) in query.iter() {
        if xform.translation.y < min_y
            || xform.translation.y > max_y
            || xform.translation.x < min_x
            || xform.translation.x > max_x
        {
            commands.entity(e).despawn();
        }
    }
}

fn update_scoreboard(scoreboard: Res<Scoreboard>, mut query: Query<&mut Text, With<ScoreboardUi>>) {
    let mut text = query.single_mut();
    text.sections[1].value = scoreboard.score.to_string();
}

fn check_for_collisions(
    mut commands: Commands,
    mut scoreboard: ResMut<Scoreboard>,
    collider_query: Query<(Entity, &Transform), (With<Collider>, Without<Player>)>,
    player_query: Query<&Transform, With<Player>>,
    mut collision_events: EventWriter<CollisionEvent>,
) {
    let player_transform = player_query.single();
    let player_bb = Aabb2d::new(
        player_transform.translation.truncate(),
        player_transform.scale.truncate() / 2.,
    );

    // check collision with walls
    for (e, other_transform) in &collider_query {
        let was_collision =
            BoundingCircle::new(other_transform.translation.truncate(), BALL_DIAMETER / 2.)
                .intersects(&player_bb);

        if was_collision {
            info!("Collision!");
            collision_events.send_default();
            commands.entity(e).despawn();
        }
    }
}
