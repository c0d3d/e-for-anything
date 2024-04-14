//! A simplified implementation of the classic game "Breakout".
use std::f32::consts::TAU;

use bevy::math::bounding::Aabb2d;
use bevy::math::bounding::IntersectsVolume;
use bevy::prelude::*;
use bevy_turborand::prelude::*;

const INIT_DIFFICULTY: f64 = 0.05;
const STARTING_HEALTH: usize = 5;
const DIFFICULTY_INCREMENT: f64 = 0.001;

const PLAYER_SPEED: f32 = 500.0;
const METEOR_SPEED: f32 = 250.0;

// Small meteor dimensions
const SMALL_METEOR_WIDTH: f32 = 108.;
const SMALL_METEOR_HEIGHT: f32 = 92.;
const SMALL_METEOR_VEC: Vec2 = Vec2::new(SMALL_METEOR_WIDTH, SMALL_METEOR_HEIGHT);
const SMALL_METEOR_SCALE: f32 = 0.5;

// Big meteor dimensions
const BIG_METEOR_WIDTH: f32 = 192.;
const BIG_METEOR_HEIGHT: f32 = 156.;
const BIG_METEOR_VEC: Vec2 = Vec2::new(BIG_METEOR_WIDTH, BIG_METEOR_HEIGHT);
const BIG_METEOR_SCALE: f32 = 0.5;

const SCOREBOARD_FONT_SIZE: f32 = 40.0;
const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);

const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);
const TEXT_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const SCORE_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.75, 0.35);

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum GameState {
    InGame,
    Dead,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct GamePlaySet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct DeadScreenSet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(
            // Required to upload to itch, need to run at least once.
            AssetPlugin {
                mode: AssetMode::Processed,
                ..default()
            }
        ))
        .insert_state(GameState::InGame)
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .init_resource::<GlobalRng>()
        .add_event::<CollisionEvent>()
        .add_event::<DeathEvent>()
        .add_systems(
            Update,
            (
                despawn_offscreen,
                // Might need chaining?
                move_player,
                apply_velocity,
                check_for_collisions,
                apply_damage,
                apply_rotations,
                update_health_bar,
                animate_sprites,
                update_scoreboard,
                handle_death,
            )
                .in_set(GamePlaySet),
        )
        .add_systems(Startup, init)
        .add_systems(
            FixedUpdate,
            (maybe_spawn_meteor, increase_difficulty).in_set(GamePlaySet),
        )
        .add_systems(Update, (retry_button_system).in_set(DeadScreenSet))
        .add_systems(
            Update,
            (
                // Always want this one
                bevy::window::close_on_esc,
            ),
        )
        .add_systems(OnEnter(GameState::InGame), setup)
        .add_systems(OnEnter(GameState::Dead), on_death_enter)
        .add_systems(OnExit(GameState::Dead), on_death_exit)
        .configure_sets(Update, (GamePlaySet.run_if(in_state(GameState::InGame)),))
        .configure_sets(
            FixedUpdate,
            (GamePlaySet.run_if(in_state(GameState::InGame)),),
        )
        .configure_sets(Update, (DeadScreenSet.run_if(in_state(GameState::Dead)),))
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
#[component(storage = "SparseSet")]
struct Meteor;

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
struct Velocity(Vec2);

#[derive(Component)]
#[component(storage = "SparseSet")]
struct Collider(Vec2);

#[derive(Component, Deref, DerefMut)]
struct Health(usize);

#[derive(Event, Default)]
struct CollisionEvent;

#[derive(Event, Default)]
struct DeathEvent;

/// Percentage difficulty, represents chance of meteor spawning in given FixedUpdate tick
#[derive(Resource)]
struct Difficulty(f64, Timer);

struct MeteorType {
    texture: Handle<Image>,
    dimensions: Vec2,
    scale: f32,
}

#[derive(Resource)]
struct MeteorRes {
    types: Vec<MeteorType>,
}

#[derive(Component)]
struct RotationalMomentum(f32);

#[derive(Component)]
struct AnimationTimer(Timer, usize, usize);

// This resource tracks the game's score
#[derive(Resource)]
struct Scoreboard {
    score: usize,
    tick_timer: Timer,
}

#[derive(Component)]
struct ScoreboardUi;

#[derive(Component)]
struct HealthBarUi(usize);

fn bottom(w: &Window) -> f32 {
    return w.height() / -2.;
}

fn left(w: &Window) -> f32 {
    return w.width() / -2.;
}

fn right(w: &Window) -> f32 {
    return w.width() / 2.;
}

fn animate_sprites(
    time: Res<Time>,
    mut query: Query<(&mut AnimationTimer, &mut TextureAtlas)>,
) {
    for (mut timer, mut atlas) in &mut query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            atlas.index = if atlas.index >= timer.1 {
                timer.2
            } else {
                atlas.index + 1
            };
        }
    }
}

fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Start at 20% chance of spawning meteor
    commands.insert_resource(Difficulty(
        INIT_DIFFICULTY,
        Timer::from_seconds(0.25, TimerMode::Repeating),
    ));

    commands.insert_resource(MeteorRes {
        types: vec![
            MeteorType {
                texture: asset_server.load("spr_meteor_big.png"),
                dimensions: BIG_METEOR_VEC,
                scale: BIG_METEOR_SCALE,
            },
            MeteorType {
                texture: asset_server.load("spr_meteor_small.png"),
                dimensions: SMALL_METEOR_VEC,
                scale: SMALL_METEOR_SCALE,
            },
        ],
    });

    commands.insert_resource(Scoreboard {
        score: 0,
        tick_timer: Timer::from_seconds(0.25, TimerMode::Repeating),
    })
}

// Add the game's entities to our world
fn setup(
    mut commands: Commands,
    mut rng: ResMut<GlobalRng>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    window: Query<&Window>,
    mut difficulty: ResMut<Difficulty>,
    mut scoreboard: ResMut<Scoreboard>,
) {
    difficulty.0 = INIT_DIFFICULTY;
    scoreboard.score = 0;

    let player_normal_anim = asset_server.load("PlayerSheetNormal.png");
    let atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        Vec2::new(150., 250.),
        5,
        1,
        None,
        None,
    ));

    commands.spawn((
        SpriteSheetBundle {
            transform: Transform {
                translation: Vec3::new(0.0, bottom(window.single()) + 30., 0.0),
                scale: Vec3::new(0.2, 0.2, 0.),
                ..default()
            },
            texture: player_normal_anim,
            atlas: TextureAtlas {
                layout: atlas_layout,
                index: 1,
            },
            ..default()
        },
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating), 4, 0),
        Player,
        Collider(Vec2::new(150., 250.)),
        RngComponent::from(&mut rng),
        Health(STARTING_HEALTH),
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

    let heart_image = asset_server.load("Heart.png");

    // To health total
    for i in 0..STARTING_HEALTH {
        commands.spawn((
            HealthBarUi(i),
            ImageBundle {
                image: UiImage::new(heart_image.clone()),
                style: Style {
                    height: Val::Px(64.),
                    width: Val::Px(64.),
                    position_type: PositionType::Absolute,
                    left: Val::Px(
                        (25.0 * STARTING_HEALTH as f32)
                            + (-25. * (STARTING_HEALTH - i) as f32),
                    ),
                    bottom: Val::Px(0.),
                    ..default()
                },
                ..default()
            },
        ));
    }
}

fn retry_button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                next_state.set(GameState::InGame);
                *color = PRESSED_BUTTON.into();
                border_color.0 = Color::RED;
            },
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
                border_color.0 = Color::WHITE;
            },
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
                border_color.0 = Color::BLACK;
            },
        }
    }
}

fn on_death_exit(
    mut commands: Commands,
    entities: Query<Entity, (Without<Window>, Without<Camera>)>,
) {
    // Just wipe eveything (excludes window and camera)
    for e in entities.iter() {
        commands.entity(e).despawn();
    }
}

fn on_death_enter(
    mut commands: Commands,
    entities: Query<Entity, (Without<Window>, Without<Camera>)>,
    window: Query<&Window>,
    scoreboard: Res<Scoreboard>,
) {
    // Just wipe eveything (excludes window and camera)
    for e in entities.iter() {
        commands.entity(e).despawn();
    }

    let w = window.single();
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent.spawn(
                TextBundle::from_sections([TextSection::new(
                    format!("YOUR SCORE: {}", scoreboard.score),
                    TextStyle {
                        font_size: SCOREBOARD_FONT_SIZE,
                        color: SCORE_COLOR,
                        ..default()
                    },
                )])
                .with_text_justify(JustifyText::Center)
                .with_style(Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px((w.height() / 2.) - 125.),
                    left: Val::Px((w.width() / 2.) - 125.),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                }),
            );
            parent
                .spawn(ButtonBundle {
                    style: Style {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        border: UiRect::all(Val::Px(5.0)),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    border_color: BorderColor(Color::BLACK),
                    background_color: NORMAL_BUTTON.into(),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        "Retry",
                        TextStyle {
                            font_size: 40.0,
                            color: Color::rgb(0.9, 0.9, 0.9),
                            ..default()
                        },
                    ));
                });
        });
}

fn handle_death(
    mut collisions: EventReader<DeathEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for _ in collisions.read() {
        next_state.set(GameState::Dead);
    }
}

fn update_health_bar(
    player_health: Query<&Health, With<Player>>,
    hearts: Query<(Entity, &HealthBarUi)>,
    mut commands: Commands,
) {
    let cur_health = player_health.single().0;
    for (heart_entity, HealthBarUi(idx)) in hearts.iter() {
        if *idx >= cur_health {
            commands.entity(heart_entity).despawn();
        }
    }
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
        player_transform.translation.x + direction * PLAYER_SPEED * time.delta_seconds();
    let w = window.single();
    player_transform.translation.x = player_position.clamp(left(w), right(w));
}

fn increase_difficulty(mut difficulty: ResMut<Difficulty>, time: Res<Time>) {
    difficulty.1.tick(time.delta());

    if difficulty.1.finished() {
        difficulty.0 += DIFFICULTY_INCREMENT;
    }
}

// TODO SpawnMeteorEvent
fn maybe_spawn_meteor(
    difficulty: Res<Difficulty>,
    window: Query<&Window>,
    mut commands: Commands,
    meteor_types: Res<MeteorRes>,
    mut rng: Query<&mut RngComponent, With<Player>>,
) {
    let mut c_rng = rng.single_mut();
    let meteor_type = c_rng.sample(meteor_types.types.as_slice()).expect("A type");
    let w = window.single();
    if c_rng.chance(difficulty.0.clamp(0.0, 100.0)) {
        commands.spawn((
            Meteor,
            Collider(meteor_type.dimensions.clone()),
            Velocity(
                Vec2 {
                    x: c_rng.f32_normalized() / 15.,
                    y: -c_rng.f32(),
                }
                .normalize()
                    * METEOR_SPEED,
            ),
            SpriteSheetBundle {
                transform: Transform::from_translation(Vec3::new(
                    c_rng.i32((left(w) as i32)..(right(w) as i32)) as f32,
                    window.single().height() / 2.,
                    1.0,
                ))
                .with_scale(Vec2::splat(meteor_type.scale).extend(1.)),
                texture: meteor_type.texture.clone(),
                ..default()
            },
            RotationalMomentum(c_rng.f32_normalized()),
        ));
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

fn apply_rotations(
    mut query: Query<(&mut Transform, &RotationalMomentum)>,
    timer: Res<Time>,
) {
    for (mut t, RotationalMomentum(ref s)) in query.iter_mut() {
        t.rotate_z(s * TAU * timer.delta_seconds());
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

fn update_scoreboard(
    mut scoreboard: ResMut<Scoreboard>,
    time: Res<Time>,
    mut query: Query<&mut Text, With<ScoreboardUi>>,
) {
    scoreboard.tick_timer.tick(time.delta());

    if scoreboard.tick_timer.finished() {
        scoreboard.score += 1;
    }

    let mut text = query.single_mut();
    text.sections[1].value = scoreboard.score.to_string();
}

fn apply_damage(
    mut collisions: EventReader<CollisionEvent>,
    mut player: Query<&mut Health, With<Player>>,
    mut death_events: EventWriter<DeathEvent>,
) {
    for _ in collisions.read() {
        if player.single_mut().0 > 0 {
            player.single_mut().0 -= 1;
            if player.single().0 == 0 {
                death_events.send_default();
            }
        } else {
            warn!("You should be dead!");
        }
    }
}

fn check_for_collisions(
    mut commands: Commands,
    collider_query: Query<(Entity, &Collider, &Transform), Without<Player>>,
    player_query: Query<(&Collider, &Transform), With<Player>>,
    mut collision_events: EventWriter<CollisionEvent>,
) {
    let (player_collider, player_transform) = player_query.single();
    let player_bb = Aabb2d::new(
        player_transform.translation.truncate(),
        (player_transform.scale.truncate() * player_collider.0) / 2.,
    );

    // check collision with walls
    for (e, collider, other_transform) in &collider_query {
        let was_collision = Aabb2d::new(
            other_transform.translation.truncate(),
            (other_transform.scale.truncate() * collider.0) / 2.,
        )
        .intersects(&player_bb);

        if was_collision {
            info!("Collision!");
            collision_events.send_default();
            commands.entity(e).despawn();
        }
    }
}
